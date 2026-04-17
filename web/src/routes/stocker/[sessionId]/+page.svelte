<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { page } from '$app/state';
	import { goto, beforeNavigate } from '$app/navigation';
	import { api, QueuedError, ApiClientError } from '$api/client.js';
	import type { BarcodeResolution, CameraToken, Item, ItemSummary, ScanSession, StoredEvent, StockerBatchEvent, ExternalCode } from '$api/types.js';
	import { detectBarcodeType, STANDARD_CODE_TYPES, STANDARD_CODE_TYPE_VALUES } from '$lib/barcode-type.js';
	import QRCode from 'qrcode';
	import { onScan, scannerState, startSerialScanner, startCameraScanner, startHidScanner, stopScanner } from '$scanner/index.js';
	import { scanSuccess, scanError, contextSet, newItem as newItemSound } from '$audio/feedback.js';
	import { init as initAudio } from '$audio/feedback.js';
	import {
		stockerStore,
		setSession,
		setContext,
		addRecentItem,
		markSynced,
		setPendingCount,
		clearSession
	} from '$stores/stocker.js';
	import {
		getRecentContainers,
		pushRecentContainer,
		type RecentContainer
	} from '$stores/recentContainers.js';
	import ScannerSettingsModal from './ScannerSettingsModal.svelte';
	import ActiveItemPanel from './ActiveItemPanel.svelte';
	import QuickCreatePanel from './QuickCreatePanel.svelte';
	import PlaceContainerModal from './PlaceContainerModal.svelte';
	import CameraLinkPanel from './CameraLinkPanel.svelte';
	import ScanLog from './ScanLog.svelte';
	import { eventTypeToMessage } from './eventTypeToMessage.js';

	let sessionId = $derived(page.params.sessionId!);

	// ── State ────────────────────────────────────────────────────────────────
	interface ScanLogEntry {
		id: number;
		barcode: string;
		type: 'success' | 'context' | 'create' | 'error';
		message: string;
		item?: Item;
		itemId?: string;
		itemName?: string;   // snapshot at event time — fallback when cache misses
		parentId?: string;
		parentName?: string; // snapshot at event time
		imageUrl?: string;
		timestamp: number;
	}

	let scanLog: ScanLogEntry[] = $state([]);
	let logIdCounter: number = $state(0);
	// itemId → current display name. Populated from history load, creation
	// paths, and SSE ItemUpdated echoes so renames reflect live in the log.
	let nameCache: Record<string, string> = $state({});

	function cacheName(id: string | undefined | null, name: string | undefined | null) {
		if (!id || !name) return;
		if (nameCache[id] === name) return;
		nameCache[id] = name;
	}

	/** Compose the display text for a log entry using the current nameCache. */
	function entryText(e: ScanLogEntry): string {
		if (!e.itemId) return e.message;
		const name = nameCache[e.itemId] ?? e.itemName ?? '';
		const parent = (e.parentId && nameCache[e.parentId]) ?? e.parentName;
		if (!name) return e.message;
		if (e.imageUrl) return `Photo added: ${name}`;
		if (e.type === 'create') return parent ? `Created: ${name} → ${parent}` : `Created: ${name}`;
		if (e.message.startsWith('Moved: ') || e.message.startsWith('Moved container: ')) {
			const prefix = e.message.startsWith('Moved container: ') ? 'Moved container' : 'Moved';
			return parent ? `${prefix}: ${name} → ${parent}` : `${prefix}: ${name}`;
		}
		if (e.message.startsWith('Queued: ')) {
			return parent ? `Queued: ${name} → ${parent}` : `Queued: ${name}`;
		}
		if (e.message.startsWith('Updated: ')) return `Updated: ${name}`;
		if (e.message.startsWith('Deleted: ')) return `Deleted: ${name}`;
		return e.message;
	}
	let loading: boolean = $state(true);
	let ending: boolean = $state(false);
	let error: string = $state('');
	let activeItemName: string = $state('');
	let lightboxUrl: string | null = $state(null);

	let pendingBatch: StockerBatchEvent[] = $state([]);
	let flushTimer: ReturnType<typeof setInterval> | null = $state(null);
	let eventSource: EventSource | null = $state(null);
	let flushing: boolean = $state(false);
	const FLUSH_INTERVAL_MS = 2000;
	let seenEventIds: Set<string> = new Set();

	// SC-2: Track in-flight barcodes to prevent duplicate processing when the
	// same barcode fires twice before the first async resolve completes.
	const inFlight = new Set<string>();

	// Quick-create panel
	let showQuickCreate: boolean = $state(false);
	let qcName: string = $state('');
	let qcQuantity: number = $state(1);
	let qcBarcode: string = $state('');           // system barcode (HOM- prefix or unknown)
	let qcExternalCode: ExternalCode | null = $state(null); // detected UPC/EAN/ISBN
	let qcLoading: boolean = $state(false);
	let qcError: string = $state('');

	// Container picker
	let showContainerPicker: boolean = $state(false);
	let pickerQuery: string = $state('');
	let pickerResults: ItemSummary[] = $state([]);
	let pickerLoading: boolean = $state(false);
	let pickerRecents: RecentContainer[] = $state([]);

	// Container move mode: when active, scanning a container moves it into the active
	// context instead of setting it as the new active container.
	let containerMoveMode: boolean = $state(false);

	// Load recents and all containers whenever the picker opens
	$effect(() => {
		if (showContainerPicker) {
			pickerRecents = getRecentContainers();
			pickerQuery = '';
			loadAllContainers();
		}
	});

	async function loadAllContainers() {
		pickerLoading = true;
		try {
			pickerResults = await api.search.query({ q: '', is_container: true, limit: 50 });
		} catch {
			pickerResults = [];
		} finally {
			pickerLoading = false;
		}
	}

	// Scanner modal
	let showScannerSettings: boolean = $state(false);

	// Active item mini-panel
	let showItemPanel: boolean = $state(false);
	let panelItem: Item | null = $state(null);
	let panelLoading: boolean = $state(false);
	let panelError: string = $state('');

	async function openItemPanel(itemId: string) {
		showItemPanel = true;
		panelLoading = true;
		panelError = '';
		panelItem = null;
		try {
			panelItem = await api.items.get(itemId);
		} catch (err) {
			panelError = err instanceof Error ? err.message : 'Failed to load item';
		} finally {
			panelLoading = false;
		}
	}

	// Camera scanner preview (when source === 'camera')
	let cameraVideoEl: HTMLVideoElement | null = $state(null);
	let cameraContainer: HTMLDivElement | null = $state(null);

	$effect(() => {
		if (!cameraContainer || !cameraVideoEl) return;
		cameraVideoEl.className = 'w-full max-h-56 object-cover';
		cameraContainer.appendChild(cameraVideoEl);
		return () => {
			if (cameraVideoEl?.parentNode === cameraContainer) cameraContainer?.removeChild(cameraVideoEl);
		};
	});

	async function pickCameraScanner() {
		showScannerSettings = false;
		if (!navigator.mediaDevices?.getUserMedia) {
			addLog('camera', 'error', 'Camera requires HTTPS or localhost');
			return;
		}
		try {
			const vid = await startCameraScanner();
			cameraVideoEl = vid ?? null;
		} catch {
			addLog('camera', 'error', 'Camera permission denied');
		}
	}

	// Container placement modal (for container presets)
	let showPlaceContainer: boolean = $state(false);
	let placeContainerBarcode: string = $state('');
	let placeContainerTypeId: string | null = $state(null);
	let placeContainerTypeName: string | null = $state(null);
	let placeParentQuery: string = $state('');
	let placeParentResults: ItemSummary[] = $state([]);
	let placeParentLoading: boolean = $state(false);
	let placeParentSelected: ItemSummary | Item | null = $state(null);
	let placingContainer: boolean = $state(false);
	let placeError: string = $state('');
	let suggestedParent: Item | null = $state(null);

	// Camera link
	let showCameraLink: boolean = $state(false);
	let cameraTokens: CameraToken[] = $state([]);
	let cameraLinkLoading: boolean = $state(false);
	let cameraLinkError: string = $state('');
	let cameraDeviceName: string = $state('');
	let cameraQrCodes: Record<string, string> = $state({});

	let context = $derived($stockerStore.context);
	let session = $derived($stockerStore.session);

	// ── Lifecycle ────────────────────────────────────────────────────────────
	onMount(async () => {
		try {
			const s = await api.stocker.getSession(sessionId);
			if (s.ended_at) {
				error = 'This session has already ended.';
				loading = false;
				return;
			}
			setSession(s);

			// Restore container context from server state on page load/refresh
			if (s.active_container_id && !context.containerId) {
				try {
					const item = await api.items.get(s.active_container_id);
					setContext({
						containerId: s.active_container_id,
						containerName: item.name ?? 'Unnamed'
					});
				} catch {
					// Container may have been deleted; proceed without context
				}
			}

			// Fetch active item name
			if (s.active_item_id) {
				try {
					const item = await api.items.get(s.active_item_id);
					activeItemName = item.name ?? '';
				} catch { /* ignore */ }
			}

			// Load recent session history into scan log
			await loadSessionHistory(s);
		} catch {
			error = 'Session not found.';
			loading = false;
			return;
		}
		loading = false;

		// Start flush timer
		flushTimer = setInterval(flushBatch, FLUSH_INTERVAL_MS);

		// Connect SSE stream for real-time updates
		connectEventStream();
	});

	// H-11: Kick off a best-effort flush on navigation. We don't cancel —
	// the offline queue persists any unsent events to IndexedDB so they
	// won't be lost even if the user closes the tab mid-flush.
	beforeNavigate(() => {
		if (pendingBatch.length > 0) void flushBatch();
	});

	onDestroy(() => {
		if (flushTimer) clearInterval(flushTimer);
		if (eventSource) { eventSource.close(); eventSource = null; }
		unregisterScan();
		stopScanner();
	});

	const unregisterScan = onScan(handleScan);

	// ── Container context helpers ────────────────────────────────────────────

	/** Set a container as the active context, push to recent list, and queue the batch event. */
	function setActiveContainer(id: string, name: string | null, containerPath: string | null, parentName: string | null = null) {
		setContext({ containerId: id, containerName: name ?? 'Unnamed' });
		pendingBatch.push({ type: 'set_context', container_id: id, scanned_at: new Date().toISOString() });
		setPendingCount(pendingBatch.length);
		pushRecentContainer({ id, name: name ?? 'Unnamed', container_path: containerPath, parent_name: parentName });
		containerMoveMode = false;
		addLog(name ?? id, 'context', `Context → ${name ?? 'Unnamed'}`);
		contextSet();
		showContainerPicker = false;
		pickerQuery = '';
		pickerResults = [];
	}

	// ── Scan handling ────────────────────────────────────────────────────────
	async function handleScan(event: { barcode: string }) {
		initAudio(); // ensure AudioContext is live after user interaction
		const barcode = event.barcode.trim().toUpperCase();
		if (!barcode) return;

		// Prevent double-scan of the same barcode within 500ms
		const now = Date.now();
		if (scanLog.length > 0 && scanLog[0].barcode === barcode && now - scanLog[0].timestamp < 500) return;

		// SC-2: Prevent duplicate processing from rapid-fire scan events that
		// arrive while the first async resolve is still in-flight.
		if (inFlight.has(barcode)) return;
		inFlight.add(barcode);

		try {
			await handleScanInner(barcode);
		} finally {
			inFlight.delete(barcode);
		}
	}

	async function handleScanInner(barcode: string) {
		// If the container picker is open, intercept the scan and use it to pick a container.
		if (showContainerPicker) {
			try {
				const res = await api.barcodes.resolve(barcode);
				if (res.type === 'system') {
					const item = await api.items.get(res.item_id);
					if (item.is_container) {
						setActiveContainer(item.id, item.name, item.container_path ?? null);
					} else {
						addLog(barcode, 'error', 'Not a container');
					}
				}
			} catch { /* silently ignore resolve errors while picker is open */ }
			return;
		}

		let resolution: BarcodeResolution;
		try {
			resolution = await api.barcodes.resolve(barcode);
		} catch (err) {
			addLog(barcode, 'error', `Resolve failed: ${err instanceof Error ? err.message : 'Unknown error'}`);
			scanError();
			return;
		}

		switch (resolution.type) {
			case 'system': {
				let item: Item;
				try {
					item = await api.items.get(resolution.item_id);
				} catch {
					addLog(barcode, 'error', 'Failed to fetch item details');
					scanError();
					return;
				}
				if (item.is_container) {
					if (containerMoveMode) {
						// Move mode: move the container itself into the active context
						if (!context.containerId) {
							addLog(barcode, 'error', 'Set a container context first');
							scanError();
							return;
						}
						pendingBatch.push({
							type: 'move_item',
							item_id: item.id,
							scanned_at: new Date().toISOString()
						});
						setPendingCount(pendingBatch.length);
						addLog(barcode, 'success', `Moved container: ${item.name ?? 'Unnamed'} → ${context.containerName}`, undefined, {
							itemId: item.id,
							itemName: item.name ?? undefined,
							parentId: context.containerId ?? undefined,
							parentName: context.containerName ?? undefined
						});
						scanSuccess();
					} else {
						// Default: set as active context
						setActiveContainer(item.id, item.name, item.container_path ?? null);
					}
				} else {
					// Move item into current context
					if (!context.containerId) {
						addLog(barcode, 'error', 'No container context set — scan a container first');
						scanError();
						return;
					}
					pendingBatch.push({
						type: 'move_item',
						item_id: item.id,
						scanned_at: new Date().toISOString()
					});
					setPendingCount(pendingBatch.length);
					addLog(barcode, 'success', `Queued: ${item.name ?? 'Unnamed'} → ${context.containerName}`, undefined, {
						itemId: item.id,
						itemName: item.name ?? undefined,
						parentId: context.containerId ?? undefined,
						parentName: context.containerName ?? undefined
					});
					addRecentItem(item);
					activeItemName = item.name ?? '';
					scanSuccess();
				}
				break;
			}

			case 'preset': {
				if (resolution.is_container) {
					// Container preset — open placement modal so user picks parent location
					placeContainerBarcode = resolution.barcode;
					placeContainerTypeId = resolution.container_type_id;
					placeContainerTypeName = resolution.container_type_name;
					placeParentQuery = '';
					placeParentResults = [];
					placeParentSelected = null;
					placeError = '';
					suggestedParent = null;
					// If there's an active container, fetch its parent to suggest
					if (context.containerId) {
						try {
							const activeItem = await api.items.get(context.containerId);
							if (activeItem.parent_id) {
								suggestedParent = await api.items.get(activeItem.parent_id);
							}
						} catch {
							// Ignore errors fetching parent; suggest will just be null
						}
					}
					showPlaceContainer = true;
					newItemSound();
					addLog(barcode, 'create', `New container: ${resolution.container_type_name ?? 'Container'}`);
				} else {
					// Item preset — auto-create in active context, no prompt
					if (!context.containerId) {
						addLog(barcode, 'error', 'No container context — scan a container first');
						scanError();
						break;
					}
					try {
						const batchRes = await api.stocker.submitBatch(sessionId, { events: [{
							type: 'create_and_place',
							barcode,
							name: barcode,
							scanned_at: new Date().toISOString()
						}] }, true);
						markBatchEventsSeen(batchRes.results);
						const created = batchRes.results.find(r => r.type === 'created') as ({ type: 'created'; item_id: string } | undefined);
						if (created) {
							const createdItem = await api.items.get(created.item_id);
							addRecentItem(createdItem);
							activeItemName = createdItem.name ?? barcode;
							addLog(barcode, 'create', `Created: ${barcode} → ${context.containerName}`, undefined, {
								itemId: created.item_id,
								itemName: createdItem.name ?? barcode,
								parentId: context.containerId ?? undefined,
								parentName: context.containerName ?? undefined
							});
							scanSuccess();
						} else {
							addLog(barcode, 'error', batchRes.errors?.[0]?.message ?? 'Create failed');
							scanError();
						}
					} catch (err) {
						if (err instanceof QueuedError) {
							addLog(barcode, 'error', 'Queued for sync (offline)');
						} else {
							addLog(barcode, 'error', err instanceof Error ? err.message : 'Create failed');
							scanError();
						}
					}
				}
				break;
			}

			case 'unknown_system': {
				// HOM- prefix not registered as a preset or existing item.
				// Items are only created from preset scans in the stocker flow.
				addLog(barcode, 'error', 'Not a registered preset — assign via item detail or create a preset first');
				scanError();
				break;
			}

			case 'unknown': {
				// Non-system barcode — add as external code to the active item.
				const activeId = $stockerStore.activeItemId;
				if (!activeId) {
					addLog(barcode, 'error', 'No active item — scan a preset barcode first');
					scanError();
					break;
				}
				const codeType = detectBarcodeType(resolution.value) || 'BARCODE';
				try {
					await api.items.addExternalCode(activeId, codeType, resolution.value);
					addLog(barcode, 'success', `Added ${codeType} to active item`);
					scanSuccess();
				} catch (err) {
					if (err instanceof ApiClientError && err.error.status === 409) {
						addLog(barcode, 'success', `${codeType} already on this item`);
						scanSuccess();
					} else if (err instanceof QueuedError) {
						addLog(barcode, 'success', 'Queued for sync (offline)');
					} else {
						addLog(barcode, 'error', err instanceof Error ? err.message : 'Failed to add code');
						scanError();
					}
				}
				break;
			}

			case 'external': {
				// Commercial code (UPC/EAN/ISBN/etc.) — add as external code to the active item.
				const activeId = $stockerStore.activeItemId;
				if (!activeId) {
					addLog(barcode, 'error', 'No active item — scan a preset barcode first');
					scanError();
					break;
				}
				const extType = detectBarcodeType(resolution.value, resolution.code_type) || resolution.code_type;
				try {
					await api.items.addExternalCode(activeId, extType, resolution.value);
					addLog(barcode, 'success', `Added ${extType} to active item`);
					scanSuccess();
				} catch (err) {
					if (err instanceof ApiClientError && err.error.status === 409) {
						addLog(barcode, 'success', `${extType} already on this item`);
						scanSuccess();
					} else if (err instanceof QueuedError) {
						addLog(barcode, 'success', 'Queued for sync (offline)');
					} else {
						addLog(barcode, 'error', err instanceof Error ? err.message : 'Failed to add code');
						scanError();
					}
				}
				break;
			}
		}
	}

	/** Seed seenEventIds with the event_ids returned by a submitBatch call so
	 *  the SSE echo of those same events does not double-log entries that we
	 *  already appended locally via optimistic addLog. */
	function markBatchEventsSeen(results: Array<{ type: string; event_id?: string }>) {
		for (const r of results) {
			if (r.event_id) seenEventIds.add(r.event_id);
		}
	}

	function addLog(barcode: string, type: ScanLogEntry['type'], message: string, item?: Item, extra?: { itemId?: string; itemName?: string; parentId?: string; parentName?: string; imageUrl?: string; timestamp?: number }) {
		const itemId = extra?.itemId ?? item?.id;
		const itemName = extra?.itemName ?? item?.name ?? undefined;
		cacheName(itemId, itemName);
		cacheName(extra?.parentId, extra?.parentName);
		scanLog = [
			{
				id: ++logIdCounter,
				barcode,
				type,
				message,
				item,
				itemId,
				itemName,
				parentId: extra?.parentId,
				parentName: extra?.parentName,
				imageUrl: extra?.imageUrl,
				timestamp: extra?.timestamp ?? Date.now()
			},
			...scanLog
		].slice(0, 100);
	}

	// ── Load session history on mount ────────────────────────────────────────
	async function loadSessionHistory(s: ScanSession) {
		try {
			// Fetch recent events for this session from the global event log
			const allRecent = await api.events.list({ limit: 100 });
			const sessionEvents = allRecent.filter(
				e => e.metadata?.session_id === s.id
			);

			// Collect unique item ids touched in the session plus the active item,
			// and fetch per-item history for each. This recovers events that lack
			// session_id metadata (e.g. ItemImageAdded uploaded via the camera
			// link) so photos don't disappear from the log after a reload.
			const itemIds = new Set<string>();
			for (const e of sessionEvents) itemIds.add(e.aggregate_id);
			if (s.active_item_id) itemIds.add(s.active_item_id);

			const itemEventLists = await Promise.all(
				Array.from(itemIds).slice(0, 20).map(async (id) => {
					try {
						return await api.items.history(id, { limit: 5 });
					} catch {
						return [] as StoredEvent[];
					}
				})
			);
			const itemEvents: StoredEvent[] = itemEventLists.flat();

			// Merge and deduplicate by event_id, newest first
			const seen = new Set<string>();
			const merged: StoredEvent[] = [];
			for (const e of [...sessionEvents, ...itemEvents]) {
				if (!seen.has(e.event_id)) {
					seen.add(e.event_id);
					seenEventIds.add(e.event_id);
					merged.push(e);
				}
			}
			merged.sort((a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime());

			// Resolve current names for every item touched (and every parent).
			// Uses api.items.get so renames show up correctly — the `name` field
			// embedded in historical event_data is a snapshot at event time.
			const history = merged.slice(0, 50);
			const namedIds = new Set<string>();
			for (const e of history) {
				namedIds.add(e.aggregate_id);
				const data = e.event_data as Record<string, unknown>;
				if (e.event_type === 'ItemCreated' && data?.parent_id) namedIds.add(data.parent_id as string);
				if (e.event_type === 'ItemMoved' && data?.new_parent_id) namedIds.add(data.new_parent_id as string);
			}
			await Promise.all([...namedIds].map(async (id) => {
				try {
					const it = await api.items.get(id);
					cacheName(id, it.name ?? 'Unnamed');
				} catch { /* ignore — item may be deleted */ }
			}));

			// Populate scan log (iterate oldest-first so newest ends on top)
			for (const e of history.reverse()) {
				const data = e.event_data as Record<string, unknown>;
				const snapshotName = (data?.name as string) ?? (data?.item_name as string) ?? '';
				let parentId: string | undefined;

				const imageUrl = (data?.path as string) ?? (data?.url as string) ?? (data?.image_url as string) ?? undefined;

				// Extract parentId if applicable
				if (e.event_type === 'ItemCreated') parentId = (data?.parent_id as string) ?? undefined;
				if (e.event_type === 'ItemMoved') parentId = (data?.new_parent_id as string) ?? undefined;

				const parentName = parentId ? nameCache[parentId] : undefined;
				const { type, message } = eventTypeToMessage(e.event_type, snapshotName, parentName);

				addLog(
					e.aggregate_id.slice(0, 8),
					type,
					message,
					undefined,
					{
						itemId: e.aggregate_id,
						itemName: snapshotName || undefined,
						parentId,
						parentName: parentId ? nameCache[parentId] : undefined,
						imageUrl,
						timestamp: new Date(e.created_at).getTime()
					}
				);
			}
		} catch { /* ignore history load failure */ }
	}

	// ── SSE stream (real-time session updates) ──────────────────────────────
	function connectEventStream() {
		if (eventSource) eventSource.close();
		eventSource = api.stocker.streamSession(sessionId);

		eventSource.addEventListener('update', async (e: MessageEvent) => {
			try {
				const data = JSON.parse(e.data);
				// Deliberately skip replacing local session state / re-fetching
				// item names on every push — those cause mid-interaction context
				// loss (active container/item/name flicker). Locally-originated
				// actions already update state through their own handlers; the
				// only remaining purpose of this event is to inject server-side
				// echoes (photo uploads from the camera app, cross-tab edits)
				// into the scan log below.

				// Process new events into scan log
				const newEvents = (data.events ?? []) as StoredEvent[];
				for (const evt of newEvents) {
					if (seenEventIds.has(evt.event_id)) continue;
					seenEventIds.add(evt.event_id);

					const evtData = evt.event_data as Record<string, unknown>;
					const snapshotName = (evtData?.name as string) ?? (evtData?.item_name as string) ?? '';
					const imgUrl = (evtData?.path as string) ?? (evtData?.url as string) ?? (evtData?.image_url as string) ?? undefined;
					let type: 'success' | 'context' | 'create' | 'error' = 'success';
					let message = '';

					// Resolve parent name for created/moved events
					let parentName = '';
					const parentId = (evtData?.parent_id as string) ?? (evtData?.new_parent_id as string);
					if (parentId && (evt.event_type === 'ItemCreated' || evt.event_type === 'ItemMoved')) {
						try {
							parentName = (await api.items.get(parentId)).name ?? '';
							cacheName(parentId, parentName);
						} catch { /* ignore */ }
					}

					// Live rename: on ItemUpdated, fetch the current item so the cache
					// reflects new names immediately across every log line for this id.
					if (evt.event_type === 'ItemUpdated') {
						try {
							const it = await api.items.get(evt.aggregate_id);
							cacheName(evt.aggregate_id, it.name ?? 'Unnamed');
						} catch { /* ignore */ }
					}

					({ type, message } = eventTypeToMessage(evt.event_type, snapshotName, parentName));

					addLog(
						evt.aggregate_id.slice(0, 8),
						type,
						message,
						undefined,
						{
							itemId: evt.aggregate_id,
							itemName: snapshotName || undefined,
							parentId: parentId ?? undefined,
							parentName: parentName || undefined,
							imageUrl: imgUrl,
							timestamp: new Date(evt.created_at).getTime()
						}
					);
				}
			} catch { /* ignore parse errors */ }
		});

		eventSource.addEventListener('phone_scan', async (e: MessageEvent) => {
			try {
				const data = JSON.parse(e.data) as { barcode?: string };
				const barcode = data.barcode?.trim();
				if (barcode) await handleScan({ barcode });
			} catch { /* ignore */ }
		});

		eventSource.addEventListener('session_ended', () => {
			error = 'Session ended';
			if (eventSource) { eventSource.close(); eventSource = null; }
		});

		eventSource.onerror = () => {
			// Reconnect after a delay on error
			if (eventSource) { eventSource.close(); eventSource = null; }
			setTimeout(connectEventStream, 5000);
		};
	}

	// ── Batch flush ──────────────────────────────────────────────────────────
	async function flushBatch() {
		if (pendingBatch.length === 0 || flushing) return;
		flushing = true;
		const batch = [...pendingBatch];
		pendingBatch = [];
		setPendingCount(0);

		try {
			const flushed = await api.stocker.submitBatch(sessionId, { events: batch }, false);
			markBatchEventsSeen(flushed.results);
			markSynced();
			// Refresh session stats so counters stay current
			try {
				const s = await api.stocker.getSession(sessionId);
				setSession(s);
			} catch { /* ignore stats refresh failure */ }
		} catch (err) {
			if (err instanceof QueuedError) {
				// Batch accepted into offline queue — local pendingBatch already cleared
				markSynced();
			} else {
				// Re-queue failed batch at the front so ordering is preserved
				pendingBatch = [...batch, ...pendingBatch];
				setPendingCount(pendingBatch.length);
				console.error('[stocker] batch flush failed', err);
			}
		} finally {
			flushing = false;
		}
	}

	// ── Quick create ─────────────────────────────────────────────────────────
	async function quickCreate(e: SubmitEvent) {
		e.preventDefault();
		if (!context.containerId) {
			qcError = 'Set a container context before creating items.';
			return;
		}
		if (qcQuantity < 1) {
			qcError = 'Quantity must be at least 1.';
			return;
		}
		qcLoading = true;
		qcError = '';
		try {
			const batchRes = await api.stocker.submitBatch(
			sessionId,
			{ events: [{
				type: 'create_and_place',
				barcode: qcExternalCode ? '' : (qcBarcode || ''),
				external_codes: qcExternalCode ? [qcExternalCode] : undefined,
				name: qcName,
				scanned_at: new Date().toISOString(),
				is_fungible: qcQuantity > 1 ? true : undefined,
				fungible_quantity: qcQuantity > 1 ? qcQuantity : undefined
			}] },
			true
		);
			markBatchEventsSeen(batchRes.results);
			const created = batchRes.results.find(r => r.type === 'created') as ({ type: 'created'; item_id: string } | undefined);
			if (created) {
				const createdItem = await api.items.get(created.item_id);
				addRecentItem(createdItem);
				activeItemName = createdItem.name ?? qcName;
				addLog(qcBarcode || '—', 'create', `Created: ${qcName} → ${context.containerName}`, undefined, {
					itemId: created.item_id,
					itemName: createdItem.name ?? qcName,
					parentId: context.containerId ?? undefined,
					parentName: context.containerName ?? undefined
				});
				scanSuccess();
				showQuickCreate = false;
				qcName = '';
				qcBarcode = '';
				qcExternalCode = null;
				qcQuantity = 1;
			} else {
				// SC-3: The server returned ok but no 'created' result — show errors
				// instead of silently closing the panel and losing the user's input.
				const errorMsgs = batchRes.errors?.map((e) => e.message) ?? [];
				qcError = errorMsgs.length > 0 ? errorMsgs.join('; ') : 'Item was not created. Please try again.';
				scanError();
			}
		} catch (err) {
			if (err instanceof QueuedError) {
				qcError = 'Queued — will create when back online';
			} else {
				qcError = err instanceof Error ? err.message : 'Create failed';
				scanError();
			}
		} finally {
			qcLoading = false;
		}
	}

	function dismissQuickCreate() {
		showQuickCreate = false;
		qcName = '';
		qcBarcode = '';
		qcExternalCode = null;
		qcQuantity = 1;
		qcError = '';
	}

	// ── Container picker ─────────────────────────────────────────────────────
	let pickerDebounce: ReturnType<typeof setTimeout> | null = $state(null);
	function onPickerInput() {
		if (pickerDebounce) clearTimeout(pickerDebounce);
		pickerDebounce = setTimeout(searchContainers, 300);
	}

	async function searchContainers() {
		if (!pickerQuery.trim()) {
			await loadAllContainers();
			return;
		}
		pickerLoading = true;
		try {
			const res = await api.search.query({ q: pickerQuery, is_container: true, limit: 20 });
			pickerResults = res;
		} catch {
			pickerResults = [];
		} finally {
			pickerLoading = false;
		}
	}

	function pickContainer(item: ItemSummary | RecentContainer) {
		setActiveContainer(item.id, item.name, item.container_path ?? null, ('parent_name' in item ? item.parent_name : null) ?? null);
	}

	// ── Camera link management ───────────────────────────────────────────────
	async function loadCameraLinks() {
		try {
			cameraTokens = await api.stocker.listCameraLinks(sessionId);
			const qr: Record<string, string> = {};
			for (const ct of cameraTokens) {
				const url = `${getCameraUrl(ct.token)}/upload`;
				qr[ct.id] = await QRCode.toDataURL(url, { width: 192, margin: 1, errorCorrectionLevel: 'M' });
			}
			cameraQrCodes = qr;
		} catch {
			cameraTokens = [];
		}
	}

	async function createCameraLink() {
		cameraLinkLoading = true;
		cameraLinkError = '';
		try {
			await api.stocker.createCameraLink(sessionId, {
				device_name: cameraDeviceName.trim() || undefined,
				expires_in_hours: 24
			});
			cameraDeviceName = '';
			await loadCameraLinks();
		} catch (err) {
			cameraLinkError = err instanceof Error ? err.message : 'Failed to create camera link';
		} finally {
			cameraLinkLoading = false;
		}
	}

	async function revokeCameraLink(tokenId: string) {
		if (!confirm('Revoke this camera link? The phone using it will be disconnected immediately.')) return;
		try {
			await api.stocker.revokeCameraLink(sessionId, tokenId);
			await loadCameraLinks();
		} catch (err) {
			cameraLinkError = err instanceof Error ? err.message : 'Failed to revoke camera link';
		}
	}

	function getCameraUrl(token: string): string {
		// Camera URLs must reach the backend directly — the mobile app
		// can't use the Vite dev proxy. Any non-8080 port means Vite dev
		// (5173/5174/5175/...): rewrite to http://host:8080. In production
		// the frontend is served by the backend, so keep protocol+port.
		const loc = typeof window !== 'undefined' ? window.location : null;
		if (!loc) return `/api/v1/stocker/camera/${token}`;
		const isDev = loc.port !== '' && loc.port !== '8080';
		const protocol = isDev ? 'http:' : loc.protocol;
		const port = isDev ? '8080' : loc.port;
		const host = port ? `${loc.hostname}:${port}` : loc.hostname;
		return `${protocol}//${host}/api/v1/stocker/camera/${token}`;
	}

	// ── End session ──────────────────────────────────────────────────────────
	async function endSession() {
		ending = true;
		await flushBatch();
		try {
			await api.stocker.endSession(sessionId);
			// Wipe store so the next session doesn't inherit stale context /
			// activeItemId / recentItems from this one.
			clearSession();
			goto('/stocker');
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to end session';
			ending = false;
		}
	}

	// ── Container placement (preset containers) ──────────────────────────────
	let placeParentDebounce: ReturnType<typeof setTimeout> | null = null;
	function onPlaceParentInput() {
		if (placeParentDebounce) clearTimeout(placeParentDebounce);
		placeParentDebounce = setTimeout(searchPlaceParent, 300);
	}

	async function searchPlaceParent() {
		if (!placeParentQuery.trim()) { placeParentResults = []; return; }
		placeParentLoading = true;
		try {
			placeParentResults = await api.search.query({ q: placeParentQuery, is_container: true, limit: 20 });
		} catch { placeParentResults = []; }
		finally { placeParentLoading = false; }
	}

	async function confirmPlaceContainer() {
		if (!placeParentSelected) { placeError = 'Select a parent container.'; return; }
		placingContainer = true;
		placeError = '';
		try {
			// Temporarily override active container to the chosen parent, create container there,
			// then restore context to the new container.
			const parentId = placeParentSelected.id;
			const parentName = placeParentSelected.name ?? 'Unnamed';

			// Flush any pending batch first so set_context ordering is correct.
			await flushBatch();

			const batchRes = await api.stocker.submitBatch(sessionId, { events: [
				{ type: 'set_context', container_id: parentId, scanned_at: new Date().toISOString() },
				{
					type: 'create_and_place',
					barcode: placeContainerBarcode,
					name: placeContainerBarcode,
					is_container: true,
					container_type_id: placeContainerTypeId ?? undefined,
					scanned_at: new Date().toISOString()
				}
			] }, true);
			markBatchEventsSeen(batchRes.results);

			const created = batchRes.results.find(r => r.type === 'created') as ({ type: 'created'; item_id: string } | undefined);
			if (created) {
				const newContainer = await api.items.get(created.item_id);
				// Set the new container as active context
				setContext({ containerId: newContainer.id, containerName: newContainer.name ?? placeContainerBarcode });
				pendingBatch.push({ type: 'set_context', container_id: newContainer.id, scanned_at: new Date().toISOString() });
				setPendingCount(pendingBatch.length);
				addLog(placeContainerBarcode, 'context', `Created & context → ${newContainer.name ?? placeContainerBarcode} in ${parentName}`, undefined, {
					itemId: newContainer.id,
					itemName: newContainer.name ?? placeContainerBarcode,
					parentId,
					parentName
				});
				contextSet();
				showPlaceContainer = false;
			} else {
				placeError = batchRes.errors?.[0]?.message ?? 'Container creation failed';
			}
		} catch (err) {
			if (err instanceof QueuedError) {
				placeError = 'Queued — will create when back online';
			} else {
				placeError = err instanceof Error ? err.message : 'Create failed';
			}
		} finally {
			placingContainer = false;
		}
	}

</script>

<svelte:head>
	<title>Stocking — Homorg</title>
</svelte:head>

{#if loading}
	<div class="flex h-dvh items-center justify-center">
		<div class="h-8 w-8 animate-spin rounded-full border-2 border-slate-600 border-t-indigo-500"></div>
	</div>
{:else if error}
	<div class="flex h-dvh flex-col items-center justify-center gap-3 px-4">
		<p class="text-red-400">{error}</p>
		<a href="/stocker" class="btn btn-secondary">← Back to sessions</a>
	</div>
{:else}
<div class="flex h-full flex-col">

	<!-- ── Header ────────────────────────────────────────────────────────── -->
	<header class="flex items-center gap-2 border-b border-slate-800 px-3 py-2">
		<a href="/stocker" class="btn btn-icon text-slate-400" aria-label="Back">
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M15 18l-6-6 6-6" />
			</svg>
		</a>
		<h1 class="flex-1 text-base font-semibold text-slate-100 truncate">Active session</h1>

		{#if $stockerStore.pendingCount > 0}
			<span class="rounded-full bg-amber-600 px-2 py-0.5 text-xs font-medium text-white">
				{$stockerStore.pendingCount} pending
			</span>
		{/if}

		<button class="btn btn-icon text-slate-400" onclick={() => { showCameraLink = !showCameraLink; if (showCameraLink) loadCameraLinks(); }} aria-label="Camera link">
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<rect x="2" y="6" width="20" height="12" rx="2" />
				<circle cx="12" cy="12" r="3" />
			</svg>
		</button>

		<button class="btn btn-icon text-slate-400" onclick={() => (showScannerSettings = !showScannerSettings)} aria-label="Scanner settings">
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<circle cx="12" cy="12" r="3" />
				<path d="M19.07 4.93A10 10 0 0 0 4.93 19.07M4.93 4.93a10 10 0 0 0 14.14 14.14" />
			</svg>
		</button>

		<button class="btn btn-danger text-xs px-2 py-1" onclick={endSession} disabled={ending}>
			{ending ? '…' : 'End'}
		</button>
	</header>

	<!-- ── Session stats ────────────────────────────────────────────────── -->
	{#if session}
		<div class="flex items-center justify-around border-b border-slate-800 px-4 py-1.5 text-center">
			<div>
				<p class="text-xs text-slate-500">Scanned</p>
				<p class="text-sm font-semibold text-slate-200">{session.items_scanned}</p>
			</div>
			<div>
				<p class="text-xs text-slate-500">Created</p>
				<p class="text-sm font-semibold text-emerald-400">{session.items_created}</p>
			</div>
			<div>
				<p class="text-xs text-slate-500">Moved</p>
				<p class="text-sm font-semibold text-indigo-400">{session.items_moved}</p>
			</div>
			{#if session.items_errored > 0}
				<div>
					<p class="text-xs text-slate-500">Errors</p>
					<p class="text-sm font-semibold text-red-400">{session.items_errored}</p>
				</div>
			{/if}
		</div>
	{/if}

	<!-- ── Context banner ───────────────────────────────────────────────── -->
	<button
		class="flex items-center gap-3 border-b px-4 py-3 text-left transition-colors
		       {containerMoveMode
		           ? 'border-amber-800/50 bg-amber-950/40 hover:bg-amber-900/40'
		           : 'border-slate-800 hover:bg-slate-800'}"
		onclick={() => {
			if (containerMoveMode) containerMoveMode = false;
			else showContainerPicker = true;
		}}
	>
		<div class="flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-lg
		            {containerMoveMode ? 'bg-amber-500/20 text-amber-400' : 'bg-indigo-500/20 text-indigo-400'}">
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M21 8a2 2 0 0 0-1.5-1.937A2 2 0 0 0 18 5.5V5a2 2 0 0 0-2-2H8a2 2 0 0 0-2 2v.5A2 2 0 0 0 4.5 6.063 2 2 0 0 0 3 8v9a3 3 0 0 0 3 3h12a3 3 0 0 0 3-3z" />
			</svg>
		</div>
		<div class="min-w-0 flex-1">
			{#if context.containerId}
				{#if containerMoveMode}
					<p class="text-xs text-amber-400">Moving containers into</p>
					<p class="truncate font-medium text-amber-300">{context.containerName}</p>
				{:else}
					<p class="text-xs text-slate-400">Current container</p>
					<p class="truncate font-medium text-slate-100">{context.containerName}</p>
				{/if}
			{:else}
				<p class="text-sm text-slate-400">Tap to set container context</p>
				<p class="text-xs text-slate-500">or scan a container barcode</p>
			{/if}
		</div>
		{#if containerMoveMode}
			<span class="text-xs font-medium text-amber-400 flex-shrink-0">Tap to exit</span>
		{:else}
			<svg class="h-4 w-4 flex-shrink-0 text-slate-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M9 18l6-6-6-6" />
			</svg>
		{/if}
	</button>

	<!-- ── Active item ──────────────────────────────────────────────────── -->
	{#if $stockerStore.activeItemId}
		{@const activeId = $stockerStore.activeItemId}
		<button
			type="button"
			onclick={() => openItemPanel(activeId)}
			class="flex w-full items-center gap-3 border-b border-slate-800 px-4 py-2 text-left hover:bg-slate-800/60 transition-colors"
		>
			<div class="flex h-7 w-7 flex-shrink-0 items-center justify-center rounded-lg bg-emerald-500/20 text-emerald-400">
				<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4" />
				</svg>
			</div>
			<div class="min-w-0 flex-1">
				<p class="text-xs text-slate-400">Active item</p>
				<p class="truncate text-sm font-medium text-emerald-300">{activeItemName || 'Unnamed item'}</p>
			</div>
			<svg class="h-4 w-4 flex-shrink-0 text-slate-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M9 18l6-6-6-6" />
			</svg>
		</button>
	{/if}

	<!-- ── Camera preview (when camera scanner is active) ──────────────── -->
	{#if $scannerState.source === 'camera'}
		<div class="relative bg-black" bind:this={cameraContainer}>
			<div class="pointer-events-none absolute inset-0 flex items-center justify-center">
				<div class="h-32 w-64 rounded-lg border-2 border-indigo-400 opacity-70"></div>
			</div>
		</div>
	{/if}

	<!-- ── Scan log ──────────────────────────────────────────────────────── -->
	<ScanLog
		entries={scanLog}
		nameCache={nameCache}
		lightboxUrl={lightboxUrl}
		onLightboxOpen={(url) => (lightboxUrl = url)}
	/>

	<!-- ── Quick action bar ──────────────────────────────────────────────── -->
	<div class="flex gap-2 border-t border-slate-800 px-4 py-2">
		<button class="btn btn-secondary flex-1" onclick={() => { qcError = ''; showQuickCreate = true; }}>
			+ Quick create item
		</button>
		{#if context.containerId}
			<button
				class="btn flex-shrink-0 px-3 {containerMoveMode ? 'bg-amber-600 text-white hover:bg-amber-700' : 'btn-secondary'}"
				onclick={() => (containerMoveMode = !containerMoveMode)}
				title={containerMoveMode ? 'Exit move mode' : 'Rearrange containers'}
				aria-label={containerMoveMode ? 'Exit container move mode' : 'Enter container move mode'}
			>
				{#if containerMoveMode}
					<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
						<path d="M20 6L9 17l-5-5" />
					</svg>
				{:else}
					<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
						<path d="M7 16V4m0 0L3 8m4-4l4 4M17 8v12m0 0l4-4m-4 4l-4-4" />
					</svg>
				{/if}
			</button>
		{/if}
	</div>
</div>

<QuickCreatePanel
	show={showQuickCreate}
	name={qcName}
	quantity={qcQuantity}
	barcode={qcBarcode}
	externalCode={qcExternalCode}
	loading={qcLoading}
	error={qcError}
	containerName={context.containerName}
	containerSet={!!context.containerId}
	onClose={dismissQuickCreate}
	onSubmit={quickCreate}
	onNameChange={(v) => (qcName = v)}
	onQuantityChange={(v) => (qcQuantity = v)}
	onBarcodeChange={(v) => (qcBarcode = v)}
	onExternalCodeChange={(v) => (qcExternalCode = v)}
	onExternalCodeTypeChange={(type) => {
		if (qcExternalCode) qcExternalCode.type = type;
	}}
/>

<PlaceContainerModal
	show={showPlaceContainer}
	barcode={placeContainerBarcode}
	typeName={placeContainerTypeName}
	parentQuery={placeParentQuery}
	parentResults={placeParentResults}
	parentLoading={placeParentLoading}
	parentSelected={placeParentSelected}
	loading={placingContainer}
	error={placeError}
	suggestedParent={suggestedParent}
	onClose={() => (showPlaceContainer = false)}
	onParentQueryChange={(query) => {
		placeParentQuery = query;
		onPlaceParentInput();
	}}
	onParentSelect={(item) => {
		placeParentSelected = item;
		placeParentResults = [];
	}}
	onParentClear={() => {
		placeParentSelected = null;
		placeParentQuery = '';
	}}
	onSuggestedParentSelect={() => {
		placeParentSelected = suggestedParent;
		placeParentQuery = '"';
	}}
	onSubmit={confirmPlaceContainer}
/>

<!-- ── Container picker ───────────────────────────────────────────────── -->
{#if showContainerPicker}
<div class="fixed inset-0 z-50 flex flex-col bg-slate-950">
	<div class="flex items-center gap-2 border-b border-slate-800 px-3 py-2">
		<button class="btn btn-icon text-slate-400" onclick={() => (showContainerPicker = false)} aria-label="Close">
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M18 6L6 18M6 6l12 12" />
			</svg>
		</button>
		<input
			class="input flex-1"
			placeholder="Search containers…"
			bind:value={pickerQuery}
			oninput={onPickerInput}
		/>
	</div>

	<div class="flex-1 overflow-y-auto p-3">
		{#if pickerLoading}
			<div class="flex h-16 items-center justify-center">
				<div class="h-5 w-5 animate-spin rounded-full border-2 border-slate-600 border-t-indigo-500"></div>
			</div>
		{:else if !pickerQuery.trim()}
			<!-- No query: show recents then all containers -->
			{#if pickerRecents.length > 0}
				<p class="mb-2 px-1 text-xs font-medium uppercase tracking-wide text-slate-500">Recent</p>
				<div class="space-y-1 mb-4">
					{#each pickerRecents as rc (rc.id)}
						<button
							class="flex w-full items-center gap-3 rounded-lg px-3 py-3 text-left transition-colors hover:bg-slate-800"
							onclick={() => pickContainer(rc)}
						>
							<div class="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-md bg-indigo-500/20 text-indigo-400">
								<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
									<path d="M21 8a2 2 0 0 0-1.5-1.937A2 2 0 0 0 18 5.5V5a2 2 0 0 0-2-2H8a2 2 0 0 0-2 2v.5A2 2 0 0 0 4.5 6.063 2 2 0 0 0 3 8v9a3 3 0 0 0 3 3h12a3 3 0 0 0 3-3z" />
								</svg>
							</div>
							<div class="min-w-0">
								<p class="truncate font-medium text-slate-100">{rc.name}</p>
								{#if rc.parent_name}
									<p class="truncate text-xs text-slate-500">in {rc.parent_name}</p>
								{/if}
							</div>
						</button>
					{/each}
				</div>
			{/if}
			{#if pickerResults.length > 0}
				<p class="mb-2 px-1 text-xs font-medium uppercase tracking-wide text-slate-500">All containers</p>
				<div class="space-y-1">
					{#each pickerResults as item (item.id)}
						<button
							class="flex w-full items-center gap-3 rounded-lg px-3 py-3 text-left transition-colors hover:bg-slate-800"
							onclick={() => pickContainer(item)}
						>
							<div class="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-md bg-indigo-500/20 text-indigo-400 text-xs">
								<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
									<path d="M21 8a2 2 0 0 0-1.5-1.937A2 2 0 0 0 18 5.5V5a2 2 0 0 0-2-2H8a2 2 0 0 0-2 2v.5A2 2 0 0 0 4.5 6.063 2 2 0 0 0 3 8v9a3 3 0 0 0 3 3h12a3 3 0 0 0 3-3z" />
								</svg>
							</div>
							<div class="min-w-0">
								<p class="truncate font-medium text-slate-100">{item.name ?? 'Unnamed'}</p>
								{#if item.parent_name}
									<p class="truncate text-xs text-slate-500">in {item.parent_name}</p>
								{:else if item.system_barcode}
									<p class="text-xs text-slate-400 font-mono">{item.system_barcode}</p>
								{/if}
							</div>
						</button>
					{/each}
				</div>
			{/if}
		{:else if pickerResults.length === 0}
			<p class="py-8 text-center text-sm text-slate-500">No containers found</p>
		{:else}
			<div class="space-y-1">
				{#each pickerResults as item (item.id)}
					<button
						class="flex w-full items-center gap-3 rounded-lg px-3 py-3 text-left transition-colors hover:bg-slate-800"
						onclick={() => pickContainer(item)}
					>
						<div class="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-md bg-indigo-500/20 text-indigo-400 text-xs">
							<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
								<path d="M21 8a2 2 0 0 0-1.5-1.937A2 2 0 0 0 18 5.5V5a2 2 0 0 0-2-2H8a2 2 0 0 0-2 2v.5A2 2 0 0 0 4.5 6.063 2 2 0 0 0 3 8v9a3 3 0 0 0 3 3h12a3 3 0 0 0 3-3z" />
							</svg>
						</div>
						<div class="min-w-0">
							<p class="truncate font-medium text-slate-100">{item.name ?? 'Unnamed'}</p>
							{#if item.container_path}
								<p class="truncate text-xs text-slate-500">{item.container_path}</p>
							{:else if item.system_barcode}
								<p class="text-xs text-slate-400 font-mono">{item.system_barcode}</p>
							{/if}
						</div>
					</button>
				{/each}
			</div>
		{/if}
	</div>
</div>
{/if}

<CameraLinkPanel
	show={showCameraLink}
	tokens={cameraTokens}
	loading={cameraLinkLoading}
	error={cameraLinkError}
	deviceName={cameraDeviceName}
	qrCodes={cameraQrCodes}
	onClose={() => (showCameraLink = false)}
	onDeviceNameChange={(name) => (cameraDeviceName = name)}
	onCreateLink={createCameraLink}
	onRevokeLink={revokeCameraLink}
	{getCameraUrl}
/>

<ScannerSettingsModal
	show={showScannerSettings}
	onClose={() => (showScannerSettings = false)}
	onPickCamera={pickCameraScanner}
/>
{/if}

<!-- ── Active item mini panel ──────────────────────────────────────── -->
<ActiveItemPanel
	show={showItemPanel}
	item={panelItem}
	loading={panelLoading}
	error={panelError}
	{lightboxUrl}
	onClose={() => (showItemPanel = false)}
	onLightboxOpen={(url) => (lightboxUrl = url)}
/>

<!-- ── Image lightbox ──────────────────────────────────────────────── -->
{#if lightboxUrl}
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="fixed inset-0 z-[60] flex items-center justify-center bg-black/90"
	onclick={() => lightboxUrl = null}
	onkeydown={(e) => e.key === 'Escape' && (lightboxUrl = null)}
>
	<button
		class="absolute top-4 right-4 rounded-full bg-black/50 p-2 text-white hover:bg-black/80 transition-colors"
		onclick={() => lightboxUrl = null}
		aria-label="Close"
	>
		<svg class="h-6 w-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
			<path d="M18 6L6 18M6 6l12 12" />
		</svg>
	</button>
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<img
		src={lightboxUrl}
		alt="Full size"
		class="max-h-[90vh] max-w-[95vw] rounded-lg object-contain"
		onclick={(e) => e.stopPropagation()}
	/>
</div>
{/if}
