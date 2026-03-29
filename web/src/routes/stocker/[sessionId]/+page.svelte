<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { page } from '$app/state';
	import { goto, beforeNavigate } from '$app/navigation';
	import { api } from '$api/client.js';
	import type { BarcodeResolution, CameraToken, Item, ItemSummary, StockerBatchEvent } from '$api/types.js';
	import { onScan, scannerState, startSerialScanner, startCameraScanner, startHidScanner } from '$scanner/index.js';
	import { scanSuccess, scanError, contextSet, newItem as newItemSound } from '$audio/feedback.js';
	import { init as initAudio } from '$audio/feedback.js';
	import {
		stockerStore,
		setSession,
		setContext,
		addRecentItem,
		markSynced,
		setPendingCount
	} from '$stores/stocker.js';

	let sessionId = $derived(page.params.sessionId!);

	// ── State ────────────────────────────────────────────────────────────────
	interface ScanLogEntry {
		id: number;
		barcode: string;
		type: 'success' | 'context' | 'create' | 'error';
		message: string;
		item?: Item;
		timestamp: number;
	}

	let scanLog: ScanLogEntry[] = $state([]);
	let logIdCounter: number = $state(0);
	let loading: boolean = $state(true);
	let ending: boolean = $state(false);
	let error: string = $state('');

	let pendingBatch: StockerBatchEvent[] = $state([]);
	let flushTimer: ReturnType<typeof setInterval> | null = $state(null);
	let flushing: boolean = $state(false);
	const FLUSH_INTERVAL_MS = 2000;

	// SC-2: Track in-flight barcodes to prevent duplicate processing when the
	// same barcode fires twice before the first async resolve completes.
	const inFlight = new Set<string>();

	// Quick-create panel
	let showQuickCreate: boolean = $state(false);
	let qcName: string = $state('');
	let qcQuantity: number = $state(1);
	let qcBarcode: string = $state('');
	let qcLoading: boolean = $state(false);
	let qcError: string = $state('');

	// Container picker
	let showContainerPicker: boolean = $state(false);
	let pickerQuery: string = $state('');
	let pickerResults: ItemSummary[] = $state([]);
	let pickerLoading: boolean = $state(false);

	// Scanner modal
	let showScannerSettings: boolean = $state(false);

	// Camera link
	let showCameraLink: boolean = $state(false);
	let cameraTokens: CameraToken[] = $state([]);
	let cameraLinkLoading: boolean = $state(false);
	let cameraLinkError: string = $state('');
	let cameraDeviceName: string = $state('');

	let context = $derived($stockerStore.context);
	let session = $derived($stockerStore.session);

	// ── Lifecycle ────────────────────────────────────────────────────────────
	onMount(async () => {
		try {
			const s = await api.stocker.getSession(sessionId);
			setSession(s);
		} catch {
			error = 'Session not found.';
			loading = false;
			return;
		}
		loading = false;

		// Start flush timer
		flushTimer = setInterval(flushBatch, FLUSH_INTERVAL_MS);
	});

	// H-11: Flush pending batch before navigation so events aren't lost.
	// SvelteKit beforeNavigate doesn't await async callbacks, so we cancel
	// navigation, flush, and re-navigate once complete.
	let navigationTarget: URL | null = $state(null);
	beforeNavigate(({ cancel, to }) => {
		if (pendingBatch.length > 0 && !navigationTarget) {
			cancel();
			navigationTarget = to?.url ?? null;
			flushBatch().finally(() => {
				const target = navigationTarget;
				navigationTarget = null;
				if (target) goto(target.pathname + target.search);
			});
		}
	});

	onDestroy(() => {
		if (flushTimer) clearInterval(flushTimer);
		unregisterScan();
	});

	const unregisterScan = onScan(handleScan);

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
					// Set as context container
					setContext({
						containerId: item.id,
						containerName: item.name ?? 'Unnamed',
					});
					pendingBatch.push({
						type: 'set_context',
						container_id: item.id,
						scanned_at: new Date().toISOString()
					});
					setPendingCount(pendingBatch.length);
					addLog(barcode, 'context', `Context → ${item.name ?? 'Unnamed'}`);
					contextSet();
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
					addLog(barcode, 'success', `Queued: ${item.name ?? 'Unnamed'} → ${context.containerName}`);
					addRecentItem(item);
					scanSuccess();
				}
				break;
			}

			case 'unknown_system':
			case 'unknown': {
				// Unknown barcode — offer to create item
				qcBarcode = barcode;
				showQuickCreate = true;
				newItemSound();
				addLog(barcode, 'create', `New item? ${barcode}`);
				break;
			}

			case 'external': {
				if (resolution.item_ids.length === 0) {
					qcBarcode = barcode;
					showQuickCreate = true;
					newItemSound();
					addLog(barcode, 'create', `External code not assigned — create item?`);
					break;
				}
				let item: Item;
				try {
					item = await api.items.get(resolution.item_ids[0]);
				} catch {
					addLog(barcode, 'error', 'Failed to fetch item details');
					scanError();
					return;
				}
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
				addLog(barcode, 'success', `Queued: ${item.name ?? 'Unnamed'} → ${context.containerName}`);
				addRecentItem(item);
				scanSuccess();
				break;
			}
		}
	}

	function addLog(barcode: string, type: ScanLogEntry['type'], message: string, item?: Item) {
		scanLog = [
			{
				id: ++logIdCounter,
				barcode,
				type,
				message,
				item,
				timestamp: Date.now()
			},
			...scanLog
		].slice(0, 100);
	}

	// ── Batch flush ──────────────────────────────────────────────────────────
	async function flushBatch() {
		if (pendingBatch.length === 0 || flushing) return;
		flushing = true;
		const batch = [...pendingBatch];
		pendingBatch = [];
		setPendingCount(0);

		try {
			await api.stocker.submitBatch(sessionId, { events: batch }, false);
			markSynced();
			// Refresh session stats so counters stay current
			try {
				const s = await api.stocker.getSession(sessionId);
				setSession(s);
			} catch { /* ignore stats refresh failure */ }
		} catch (err) {
			// Re-queue failed batch at the front so ordering is preserved
			pendingBatch = [...batch, ...pendingBatch];
			setPendingCount(pendingBatch.length);
			console.error('[stocker] batch flush failed', err);
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
		qcLoading = true;
		qcError = '';
		try {
			const batchRes = await api.stocker.submitBatch(
			sessionId,
			{ events: [{
				type: 'create_and_place',
				barcode: qcBarcode || '',
				name: qcName,
				scanned_at: new Date().toISOString(),
				is_fungible: qcQuantity > 1 ? true : undefined,
				fungible_quantity: qcQuantity > 1 ? qcQuantity : undefined
			}] },
			true
		);
			const created = batchRes.results.find(r => r.type === 'created') as ({ type: 'created'; item_id: string } | undefined);
			if (created) {
				const createdItem = await api.items.get(created.item_id);
				addRecentItem(createdItem);
				addLog(qcBarcode || '—', 'success', `Created: ${qcName} → ${context.containerName}`);
				scanSuccess();
				showQuickCreate = false;
				qcName = '';
				qcBarcode = '';
				qcQuantity = 1;
			} else {
				// SC-3: The server returned ok but no 'created' result — show errors
				// instead of silently closing the panel and losing the user's input.
				const errorMsgs = batchRes.errors?.map((e) => e.message) ?? [];
				qcError = errorMsgs.length > 0 ? errorMsgs.join('; ') : 'Item was not created. Please try again.';
				scanError();
			}
		} catch (err) {
			qcError = err instanceof Error ? err.message : 'Create failed';
			scanError();
		} finally {
			qcLoading = false;
		}
	}

	// ── Container picker ─────────────────────────────────────────────────────
	let pickerDebounce: ReturnType<typeof setTimeout> | null = $state(null);
	function onPickerInput() {
		if (pickerDebounce) clearTimeout(pickerDebounce);
		pickerDebounce = setTimeout(searchContainers, 300);
	}

	async function searchContainers() {
		if (!pickerQuery.trim()) {
			pickerResults = [];
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

	function pickContainer(item: ItemSummary) {
		setContext({
			containerId: item.id,
			containerName: item.name ?? 'Unnamed',
		});
		pendingBatch.push({
			type: 'set_context',
			container_id: item.id,
			scanned_at: new Date().toISOString()
		});
		setPendingCount(pendingBatch.length);
		addLog(item.name ?? item.id, 'context', `Context → ${item.name ?? 'Unnamed'}`);
		contextSet();
		showContainerPicker = false;
		pickerQuery = '';
		pickerResults = [];
	}

	// ── Camera link management ───────────────────────────────────────────────
	async function loadCameraLinks() {
		try {
			cameraTokens = await api.stocker.listCameraLinks(sessionId);
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
		try {
			await api.stocker.revokeCameraLink(sessionId, tokenId);
			await loadCameraLinks();
		} catch (err) {
			cameraLinkError = err instanceof Error ? err.message : 'Failed to revoke camera link';
		}
	}

	function getCameraUrl(token: string): string {
		const base = typeof window !== 'undefined' ? window.location.origin : '';
		return `${base}/api/v1/stocker/camera/${token}`;
	}

	// ── End session ──────────────────────────────────────────────────────────
	async function endSession() {
		ending = true;
		await flushBatch();
		try {
			await api.stocker.endSession(sessionId);
			goto('/stocker');
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to end session';
			ending = false;
		}
	}

	function logClass(type: ScanLogEntry['type']) {
		return `scan-line-${type}`;
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
		class="flex items-center gap-3 border-b border-slate-800 px-4 py-3 text-left transition-colors hover:bg-slate-800"
		onclick={() => (showContainerPicker = true)}
	>
		<div class="flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-lg bg-indigo-500/20 text-indigo-400">
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M21 8a2 2 0 0 0-1.5-1.937A2 2 0 0 0 18 5.5V5a2 2 0 0 0-2-2H8a2 2 0 0 0-2 2v.5A2 2 0 0 0 4.5 6.063 2 2 0 0 0 3 8v9a3 3 0 0 0 3 3h12a3 3 0 0 0 3-3z" />
			</svg>
		</div>
		<div class="min-w-0 flex-1">
			{#if context.containerId}
				<p class="text-xs text-slate-400">Current container</p>
				<p class="truncate font-medium text-slate-100">{context.containerName}</p>
			{:else}
				<p class="text-sm text-slate-400">Tap to set container context</p>
				<p class="text-xs text-slate-500">or scan a container barcode</p>
			{/if}
		</div>
		<svg class="h-4 w-4 flex-shrink-0 text-slate-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
			<path d="M9 18l6-6-6-6" />
		</svg>
	</button>

	<!-- ── Scan log ──────────────────────────────────────────────────────── -->
	<div class="flex-1 overflow-y-auto font-mono text-sm">
		{#if scanLog.length === 0}
			<div class="flex h-32 flex-col items-center justify-center gap-1 text-slate-600">
				<p>Waiting for scans…</p>
				<p class="text-xs">Scan a container, then items</p>
			</div>
		{:else}
			{#each scanLog as entry (entry.id)}
				<div class="scan-line {logClass(entry.type)} flex items-baseline gap-3 px-4 py-2">
					<span class="w-24 flex-shrink-0 truncate text-xs opacity-60">{entry.barcode}</span>
					<span class="flex-1 truncate">{entry.message}</span>
				</div>
			{/each}
		{/if}
	</div>

	<!-- ── Quick action bar ──────────────────────────────────────────────── -->
	<div class="border-t border-slate-800 px-4 py-2">
		<button class="btn btn-secondary w-full" onclick={() => (showQuickCreate = true)}>
			+ Quick create item
		</button>
	</div>
</div>

<!-- ── Quick create panel ─────────────────────────────────────────────── -->
{#if showQuickCreate}
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="fixed inset-0 z-50 flex flex-col justify-end bg-black/60" onclick={(e) => { if (e.target === e.currentTarget) { showQuickCreate = false } }} onkeydown={(e) => e.key === 'Escape' && (showQuickCreate = false)}>
	<div class="rounded-t-2xl bg-slate-900 p-4 pb-8" role="dialog" aria-modal="true" aria-labelledby="quick-create-title">
		<div class="mb-4 flex items-center justify-between">
			<h2 id="quick-create-title" class="text-base font-semibold text-slate-100">Quick create item</h2>
			<button class="btn btn-icon text-slate-400" onclick={() => (showQuickCreate = false)} aria-label="Close">
				<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M18 6L6 18M6 6l12 12" />
				</svg>
			</button>
		</div>

		{#if !context.containerId}
			<div class="mb-3 rounded-lg bg-amber-950 px-3 py-2 text-sm text-amber-300 border border-amber-800">
				No container context set. Scan a container first.
			</div>
		{/if}

		{#if qcError}
			<div class="mb-3 rounded-lg bg-red-950 px-3 py-2 text-sm text-red-300 border border-red-800">
				{qcError}
			</div>
		{/if}

		<form class="space-y-3" onsubmit={quickCreate}>
			<div>
				<label class="mb-1 block text-sm font-medium text-slate-300" for="qc-name">Name *</label>
				<input id="qc-name" class="input" placeholder="e.g. 9V Battery" bind:value={qcName} required disabled={qcLoading} />
			</div>

			<div class="flex gap-3">
				<div class="flex-1">
					<label class="mb-1 block text-sm font-medium text-slate-300" for="qc-qty">Quantity</label>
					<input id="qc-qty" class="input" type="number" min="1" bind:value={qcQuantity} disabled={qcLoading} />
				</div>
				<div class="flex-1">
					<label class="mb-1 block text-sm font-medium text-slate-300" for="qc-barcode">Barcode</label>
					<input id="qc-barcode" class="input font-mono text-xs" placeholder="scanned" bind:value={qcBarcode} disabled={qcLoading} />
				</div>
			</div>

			<div class="pt-1 text-xs text-slate-400">
				→ Will be placed in: <span class="font-medium text-slate-200">{context.containerName ?? 'none'}</span>
			</div>

			<button type="submit" class="btn btn-primary w-full" disabled={qcLoading || !context.containerId}>
				{#if qcLoading}
					<span class="h-4 w-4 animate-spin rounded-full border-2 border-white/30 border-t-white"></span>
				{:else}
					Create & place
				{/if}
			</button>
		</form>
	</div>
</div>
{/if}

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
		{:else if pickerResults.length === 0 && pickerQuery}
			<p class="py-8 text-center text-sm text-slate-500">No containers found</p>
		{:else}
			<div class="space-y-1">
				{#each pickerResults as item (item.id)}
					<button
						class="flex w-full items-center gap-3 rounded-lg px-3 py-3 text-left transition-colors hover:bg-slate-800"
						onclick={() => pickContainer(item)}
					>
						<div class="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-md bg-indigo-500/20 text-indigo-400 text-xs">
							📦
						</div>
						<div class="min-w-0">
							<p class="truncate font-medium text-slate-100">{item.name ?? 'Unnamed'}</p>
							{#if item.system_barcode}
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

<!-- ── Camera link panel ────────────────────────────────────────────── -->
{#if showCameraLink}
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="fixed inset-0 z-50 flex flex-col justify-end bg-black/60" onclick={(e) => { if (e.target === e.currentTarget) { showCameraLink = false } }} onkeydown={(e) => e.key === 'Escape' && (showCameraLink = false)}>
	<div class="rounded-t-2xl bg-slate-900 p-4 pb-8 max-h-[80vh] overflow-y-auto" role="dialog" aria-modal="true" aria-labelledby="camera-link-title">
		<div class="mb-4 flex items-center justify-between">
			<h2 id="camera-link-title" class="text-base font-semibold text-slate-100">📷 Remote Camera</h2>
			<button class="btn btn-icon text-slate-400" onclick={() => (showCameraLink = false)} aria-label="Close">
				<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M18 6L6 18M6 6l12 12" />
				</svg>
			</button>
		</div>

		<p class="mb-3 text-sm text-slate-400">
			Link a remote camera device (e.g. Android phone) to this session. Photos taken will auto-attach to the most recently scanned item.
		</p>

		{#if cameraLinkError}
			<div class="mb-3 rounded-lg bg-red-950 px-3 py-2 text-sm text-red-300 border border-red-800">
				{cameraLinkError}
			</div>
		{/if}

		<!-- Create new link -->
		<div class="mb-4 space-y-2">
			<div class="flex gap-2">
				<input
					class="input flex-1 text-sm"
					placeholder="Device name (optional)"
					bind:value={cameraDeviceName}
					disabled={cameraLinkLoading}
				/>
				<button class="btn btn-primary text-sm px-3" onclick={createCameraLink} disabled={cameraLinkLoading}>
					{cameraLinkLoading ? '…' : 'Link'}
				</button>
			</div>
		</div>

		<!-- Active links -->
		{#if cameraTokens.length > 0}
			<div class="space-y-3">
				{#each cameraTokens as ct (ct.id)}
					<div class="rounded-lg border border-slate-700 bg-slate-800/50 p-3">
						<div class="flex items-start justify-between gap-2 mb-2">
							<div>
								<p class="text-sm font-medium text-slate-200">
									{ct.device_name ?? 'Camera device'}
								</p>
								<p class="text-xs text-slate-400">
									Expires {new Date(ct.expires_at).toLocaleString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })}
								</p>
							</div>
							<button
								class="text-xs text-red-400 hover:text-red-300"
								onclick={() => revokeCameraLink(ct.id)}
							>
								Revoke
							</button>
						</div>

						<!-- Token URL for camera app -->
						<div class="rounded bg-slate-950 p-2">
							<p class="text-xs text-slate-500 mb-1">Camera upload endpoint:</p>
							<code class="block text-xs text-emerald-400 break-all select-all">
								{getCameraUrl(ct.token)}/upload
							</code>
						</div>

						<p class="mt-2 text-xs text-slate-500">
							Use this URL in the Android camera app. Send multipart POST with a "file" field to upload images.
						</p>
					</div>
				{/each}
			</div>
		{:else}
			<p class="text-sm text-slate-500 text-center py-4">No active camera links</p>
		{/if}
	</div>
</div>
{/if}

<!-- ── Scanner settings ───────────────────────────────────────────────── -->
{#if showScannerSettings}
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="fixed inset-0 z-50 flex flex-col justify-end bg-black/60" onclick={(e) => { if (e.target === e.currentTarget) { showScannerSettings = false } }} onkeydown={(e) => e.key === 'Escape' && (showScannerSettings = false)}>
	<div class="rounded-t-2xl bg-slate-900 p-4 pb-8" role="dialog" aria-modal="true" aria-labelledby="scanner-settings-title">
		<h2 id="scanner-settings-title" class="mb-4 text-base font-semibold text-slate-100">Scanner source</h2>
		<div class="space-y-2">
			<button
				class="btn w-full justify-start gap-3"
				class:btn-primary={$scannerState.source === 'hid'}
				class:btn-secondary={$scannerState.source !== 'hid'}
				onclick={() => { startHidScanner(); showScannerSettings = false; }}
			>
				<span class="text-lg">⌨️</span>
				<span>HID keyboard wedge <span class="text-xs opacity-70">(USB/BT HID)</span></span>
			</button>
			<button
				class="btn w-full justify-start gap-3"
				class:btn-primary={$scannerState.source === 'serial'}
				class:btn-secondary={$scannerState.source !== 'serial'}
				onclick={() => { startSerialScanner(); showScannerSettings = false; }}
			>
				<span class="text-lg">🔵</span>
				<span>Bluetooth SPP / USB Serial <span class="text-xs opacity-70">(Chrome 117+)</span></span>
			</button>
			<button
				class="btn w-full justify-start gap-3"
				class:btn-primary={$scannerState.source === 'camera'}
				class:btn-secondary={$scannerState.source !== 'camera'}
				onclick={() => { startCameraScanner(); showScannerSettings = false; }}
			>
				<span class="text-lg">📷</span>
				<span>Camera <span class="text-xs opacity-70">(BarcodeDetector API)</span></span>
			</button>
		</div>

		{#if $scannerState.errorMessage}
			<p class="mt-3 text-sm text-red-400">{$scannerState.errorMessage}</p>
		{/if}
	</div>
</div>
{/if}
{/if}
