<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import { api } from '$api/client.js';
	import { toast } from '$stores/toast.js';
	import { onScan, startCameraScanner, stopScanner, scannerState } from '$scanner/index.js';
	import { getRecentContainers, pushRecentContainer } from '$stores/recentContainers.js';
	import type { Item, ItemSummary, BarcodeResolution } from '$api/types.js';
	import { CONDITION_LABELS } from '$api/types.js';
	import { detectBarcodeType, STANDARD_CODE_TYPES } from '$lib/barcode-type.js';

	type PageState = 'idle' | 'resolving' | 'found' | 'multiple' | 'not_found' | 'error';

	let pageState: PageState = $state('idle');
	let lastBarcode = $state('');
	let resolvedItem: Item | null = $state(null);
	let resolvedItems: Item[] = $state([]);   // for multi-match disambiguation
	let pageError = $state('');
	let lastResolution: BarcodeResolution | null = $state(null);
	let scannedCodeBadge = $state(''); // e.g. "ISBN · 9780590353403" shown on found card

	// Attach sub-flow (link an unrecognised barcode to an existing item)
	let showAttachSearch = $state(false);
	let attachQuery = $state('');
	let attachResults: ItemSummary[] = $state([]);
	let attachSearching = $state(false);
	let attachDebounce: ReturnType<typeof setTimeout> | null = null;
	let attaching = $state(false);
	let attachError = $state('');
	let attachTypeOverride = $state(''); // used when code type cannot be auto-detected

	// Camera
	let usingCamera = $state(false);
	let videoEl: HTMLVideoElement | null = $state(null);
	let cameraContainer: HTMLDivElement | null = $state(null);

	// Mount the scanner's video element (which has the stream) into the container div.
	// bind:this on a <video> tag would replace videoEl with a new empty element, losing the stream.
	$effect(() => {
		if (!cameraContainer || !videoEl) return;
		videoEl.className = 'w-full max-h-56 object-cover';
		cameraContainer.appendChild(videoEl);
		return () => {
			if (videoEl?.parentNode === cameraContainer) cameraContainer?.removeChild(videoEl);
		};
	});

	// Move sub-flow
	let showMovePicker = $state(false);
	let moveQuery = $state('');
	let moveResults: ItemSummary[] = $state([]);
	let moveSearching = $state(false);
	let moveDebounce: ReturnType<typeof setTimeout> | null = null;
	let moving = $state(false);
	let moveError = $state('');

	// Delete
	let confirmingDelete = $state(false);
	let deleting = $state(false);

	// Recent destinations (loaded fresh on each item)
	let recentContainers = $state(getRecentContainers());

	let unregisterScan: (() => void) | null = null;

	onMount(() => {
		unregisterScan = onScan(handleScan);
	});

	onDestroy(() => {
		unregisterScan?.();
		if (usingCamera) stopScanner();
	});

	async function handleScan(event: { barcode: string }) {
		const barcode = event.barcode.trim().toUpperCase();
		if (!barcode || pageState === 'resolving') return;

		pageState = 'resolving';
		lastBarcode = barcode;
		resolvedItem = null;
		resolvedItems = [];
		pageError = '';
		lastResolution = null;
		scannedCodeBadge = '';

		try {
			const resolution = await api.barcodes.resolve(barcode);
			lastResolution = resolution;

			if (resolution.type === 'system') {
				resolvedItem = await api.items.get(resolution.item_id);
				recentContainers = getRecentContainers();
				scannedCodeBadge = resolution.barcode;
				pageState = 'found';
			} else if (resolution.type === 'external') {
				if (resolution.item_ids.length === 0) {
					pageState = 'not_found';
				} else if (resolution.item_ids.length === 1) {
					resolvedItem = await api.items.get(resolution.item_ids[0]);
					recentContainers = getRecentContainers();
					scannedCodeBadge = `${resolution.code_type} · ${resolution.value}`;
					pageState = 'found';
				} else {
					// Multiple items share this barcode — let the user pick
					resolvedItems = await Promise.all(resolution.item_ids.map(id => api.items.get(id)));
					scannedCodeBadge = `${resolution.code_type} · ${resolution.value}`;
					pageState = 'multiple';
				}
			} else {
				// preset, unknown_system, unknown
				pageState = 'not_found';
			}
		} catch (err) {
			pageError = err instanceof Error ? err.message : 'Lookup failed';
			pageState = 'error';
		}
	}

	async function toggleCamera() {
		if (usingCamera) {
			stopScanner();
			usingCamera = false;
			videoEl = null;
		} else {
			// getUserMedia requires a secure context (HTTPS or localhost)
			if (!navigator.mediaDevices?.getUserMedia) {
				toast('Camera requires HTTPS — open the app via https:// or on the same device', 'error');
				return;
			}
			try {
				const vid = await startCameraScanner();
				if (vid) {
					videoEl = vid;
					usingCamera = true;
				} else {
					toast('Camera access failed — check that permission was granted', 'error');
				}
			} catch {
				toast('Camera permission denied', 'error');
			}
		}
	}

	function scanAnother() {
		pageState = 'idle';
		lastBarcode = '';
		resolvedItem = null;
		resolvedItems = [];
		pageError = '';
		lastResolution = null;
		scannedCodeBadge = '';
		showMovePicker = false;
		moveQuery = '';
		moveResults = [];
		moveError = '';
		confirmingDelete = false;
		showAttachSearch = false;
		attachQuery = '';
		attachResults = [];
		attachError = '';
	}

	// ── Attach barcode to item ────────────────────────────────────────────────

	/** Returns attach action info when the resolution is linkable to an existing item. */
	function getAttachInfo(): { label: string; codeType: string; value: string; isAssign: boolean } | null {
		if (!lastResolution) return null;
		if (lastResolution.type === 'external') {
			const type = detectBarcodeType(lastResolution.value, lastResolution.code_type) || lastResolution.code_type;
			return { label: `Attach as ${type} code`, codeType: type, value: lastResolution.value, isAssign: false };
		}
		if (lastResolution.type === 'unknown_system') {
			return { label: 'Assign barcode to item', codeType: '', value: lastResolution.barcode, isAssign: true };
		}
		if (lastResolution.type === 'unknown') {
			const type = detectBarcodeType(lastResolution.value);
			return { label: type ? `Attach as ${type} code` : 'Attach to item', codeType: type, value: lastResolution.value, isAssign: false };
		}
		return null; // preset — handled differently
	}

	function openAttachSearch() {
		showAttachSearch = true;
		attachQuery = '';
		attachResults = [];
		attachError = '';
		attachTypeOverride = '';
	}

	function onAttachSearch() {
		if (attachDebounce) clearTimeout(attachDebounce);
		if (!attachQuery.trim()) { attachResults = []; return; }
		attachDebounce = setTimeout(async () => {
			attachSearching = true;
			try {
				attachResults = await api.search.query({ q: attachQuery, limit: 20 });
			} catch { attachResults = []; }
			finally { attachSearching = false; }
		}, 300);
	}

	async function attachToItem(item: ItemSummary) {
		const info = getAttachInfo();
		if (!info) return;
		const codeType = info.codeType || attachTypeOverride;
		if (!info.isAssign && !codeType) {
			attachError = 'Please select a code type before attaching.';
			return;
		}
		attaching = true;
		attachError = '';
		try {
			if (info.isAssign) {
				await api.items.assignBarcode(item.id, { barcode: info.value });
			} else {
				await api.items.addExternalCode(item.id, codeType, info.value);
			}
			toast(`Barcode linked to ${item.name ?? 'item'}`, 'success');
			scanAnother();
		} catch (err) {
			attachError = err instanceof Error ? err.message : 'Attach failed';
		} finally {
			attaching = false;
		}
	}

	// ── Move ─────────────────────────────────────────────────────────────────

	function openMovePicker() {
		showMovePicker = true;
		moveQuery = '';
		moveResults = [];
		moveError = '';
	}

	function onMoveSearch() {
		if (moveDebounce) clearTimeout(moveDebounce);
		if (!moveQuery.trim()) { moveResults = []; return; }
		moveDebounce = setTimeout(async () => {
			moveSearching = true;
			try {
				moveResults = await api.search.query({ q: moveQuery, is_container: true, limit: 20 });
			} catch { moveResults = []; }
			finally { moveSearching = false; }
		}, 300);
	}

	async function moveTo(containerId: string, containerName: string, containerPath: string | null) {
		if (!resolvedItem) return;
		moving = true;
		moveError = '';
		try {
			await api.items.move(resolvedItem.id, { container_id: containerId });
			pushRecentContainer({ id: containerId, name: containerName, container_path: containerPath });
			toast(`Moved to ${containerName}`, 'success');
			showMovePicker = false;
			// Refresh item so location reflects the move
			resolvedItem = await api.items.get(resolvedItem.id);
			recentContainers = getRecentContainers();
		} catch (err) {
			moveError = err instanceof Error ? err.message : 'Move failed';
		} finally {
			moving = false;
		}
	}

	async function moveToFromResult(item: ItemSummary) {
		await moveTo(item.id, item.name ?? 'Unnamed', item.container_path);
	}

	// ── Delete ───────────────────────────────────────────────────────────────

	async function deleteItem() {
		if (!resolvedItem) return;
		deleting = true;
		try {
			await api.items.delete(resolvedItem.id);
			toast('Item deleted', 'success');
			scanAnother();
		} catch (err) {
			toast(err instanceof Error ? err.message : 'Delete failed', 'error');
		} finally {
			deleting = false;
			confirmingDelete = false;
		}
	}
</script>

<svelte:head>
	<title>Scan — Homorg</title>
</svelte:head>

<div class="flex h-full flex-col">
	<!-- Header -->
	<header class="flex items-center justify-between border-b border-slate-800 px-4 py-3">
		<h1 class="text-lg font-semibold text-slate-100">Scan</h1>
		<button
			class="flex items-center gap-1.5 text-xs {usingCamera ? 'text-indigo-400' : 'text-slate-400'} hover:text-indigo-300"
			onclick={toggleCamera}
		>
			<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M23 19a2 2 0 0 1-2 2H3a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4l2-3h6l2 3h4a2 2 0 0 1 2 2z"/>
				<circle cx="12" cy="13" r="4"/>
			</svg>
			{usingCamera ? 'Camera on' : 'Camera'}
		</button>
	</header>

	<div class="flex-1 overflow-y-auto">

		<!-- Camera preview — videoEl is the scanner's element (stream already attached).
		     We append it via $effect rather than bind:this to avoid replacing it with a new empty element. -->
		{#if usingCamera}
			<div class="relative bg-black" bind:this={cameraContainer}>
				<div class="absolute inset-0 flex items-center justify-center pointer-events-none">
					<div class="h-32 w-64 rounded-lg border-2 border-indigo-400 opacity-70"></div>
				</div>
			</div>
		{/if}

		<!-- Idle state -->
		{#if pageState === 'idle'}
			<div class="flex flex-col items-center justify-center gap-4 px-8 py-16 text-center text-slate-500">
				<svg class="h-16 w-16 opacity-40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
					<rect x="3" y="3" width="5" height="5" rx="0.5"/>
					<rect x="16" y="3" width="5" height="5" rx="0.5"/>
					<rect x="3" y="16" width="5" height="5" rx="0.5"/>
					<path d="M16 16h2v2h-2zM18 18h2v2h-2zM16 20h2"/>
					<path d="M7 3v2M3 7h2M7 16v2M3 16h2"/>
					<path d="M21 12h-2M12 3v2M12 21v-2M21 16v2"/>
				</svg>
				<div>
					<p class="text-sm font-medium text-slate-400">Ready to scan</p>
					<p class="mt-1 text-xs">Scan a barcode with a handheld scanner{usingCamera ? ' or point the camera at a barcode' : ', or tap Camera above'}</p>
				</div>
			</div>

		<!-- Resolving -->
		{:else if pageState === 'resolving'}
			<div class="flex flex-col items-center justify-center gap-3 py-16 text-slate-400">
				<div class="h-8 w-8 animate-spin rounded-full border-2 border-slate-600 border-t-indigo-500"></div>
				<p class="text-sm font-mono">{lastBarcode}</p>
			</div>

		<!-- Error -->
		{:else if pageState === 'error'}
			<div class="m-4 space-y-3">
				<div class="rounded-lg bg-red-950 border border-red-800 px-4 py-3 text-sm text-red-300">{pageError}</div>
				<button class="btn btn-secondary w-full" onclick={scanAnother}>Try again</button>
			</div>

		<!-- Not found -->
		{:else if pageState === 'not_found'}
			{@const attachInfo = getAttachInfo()}
			<div class="m-4 space-y-3">
				<div class="rounded-lg bg-slate-800 px-4 py-4 text-center">
					<p class="font-medium text-slate-200">
						{lastResolution?.type === 'unknown_system' ? 'Unassigned barcode' : 'No matching item'}
					</p>
					<p class="mt-1 font-mono text-xs text-slate-500">{lastBarcode}</p>
					{#if lastResolution?.type === 'external'}
						<span class="mt-1.5 inline-block rounded-full bg-slate-700 px-2 py-0.5 text-[10px] font-medium text-slate-400">{lastResolution.code_type}</span>
					{/if}
				</div>
				{#if attachInfo}
					<button class="btn btn-secondary w-full flex items-center justify-center gap-2" onclick={openAttachSearch}>
						<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/>
							<path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>
						</svg>
						{attachInfo.label}
					</button>
				{/if}
				<button class="btn btn-ghost w-full text-sm text-slate-500" onclick={scanAnother}>Scan another</button>
			</div>

		<!-- Multiple items share this barcode — disambiguation list -->
		{:else if pageState === 'multiple'}
			<div class="p-4 space-y-3">
				<div class="rounded-lg bg-slate-800/60 px-4 py-3 text-center">
					<p class="font-medium text-slate-200">{resolvedItems.length} items share this barcode</p>
					<p class="mt-0.5 font-mono text-xs text-slate-500">{lastBarcode}</p>
				</div>
				<div class="divide-y divide-slate-800 rounded-xl overflow-hidden">
					{#each resolvedItems as item (item.id)}
						<button
							class="flex w-full items-center gap-3 bg-slate-800/40 px-4 py-3 text-left hover:bg-slate-800 active:bg-slate-700 transition-colors"
							onclick={() => { resolvedItem = item; recentContainers = getRecentContainers(); pageState = 'found'; }}
						>
							<div class="flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-lg text-base {item.is_container ? 'bg-indigo-500/20' : 'bg-slate-700'}">
								{item.is_container ? '📦' : '🔧'}
							</div>
							<div class="min-w-0 flex-1">
								<p class="truncate font-medium text-slate-100">{item.name ?? lastBarcode}</p>
								<div class="flex items-center gap-2 mt-0.5 text-xs text-slate-400">
									{#if item.category}<span>{item.category}</span>{/if}
									{#if item.condition}
										<span class="badge badge-{item.condition}" style="font-size:0.6rem">{CONDITION_LABELS[item.condition]}</span>
									{/if}
								</div>
								{#if item.container_path}
									<p class="text-xs text-slate-500 truncate mt-0.5">📍 {item.container_path}</p>
								{/if}
							</div>
							<svg class="h-4 w-4 flex-shrink-0 text-slate-600" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
								<path d="M9 18l6-6-6-6"/>
							</svg>
						</button>
					{/each}
				</div>
				<button class="btn btn-ghost w-full text-sm text-slate-500" onclick={scanAnother}>
					Scan another
				</button>
			</div>

		<!-- Found — action sheet -->
		{:else if pageState === 'found' && resolvedItem}
			{@const item = resolvedItem}
			<div class="space-y-3 p-4">

				<!-- Item card -->
				<div class="rounded-xl bg-slate-800 p-4">
					<div class="flex items-start gap-3">
						<div class="flex h-10 w-10 flex-shrink-0 items-center justify-center rounded-lg text-xl {item.is_container ? 'bg-indigo-500/20' : 'bg-slate-700'}">
							{item.is_container ? '📦' : '🔧'}
						</div>
						<div class="min-w-0 flex-1">
							<p class="font-semibold text-slate-100 leading-tight">{item.name ?? lastBarcode}</p>
							<div class="mt-1 flex flex-wrap items-center gap-2 text-xs text-slate-400">
								{#if item.category}
									<span>{item.category}</span>
								{/if}
								{#if item.condition}
									<span class="badge badge-{item.condition}" style="font-size:0.6rem">{CONDITION_LABELS[item.condition]}</span>
								{/if}
							</div>
							{#if scannedCodeBadge}
								<p class="mt-1 font-mono text-[10px] text-slate-600">{scannedCodeBadge}</p>
							{/if}
							{#if item.container_path}
								<p class="mt-1 text-xs text-slate-500 truncate">📍 {item.container_path}</p>
							{/if}
							{#if item.tags.length > 0}
								<div class="mt-1.5 flex flex-wrap gap-1">
									{#each item.tags.slice(0, 4) as tag}
										<span class="rounded-full bg-slate-700/60 px-2 py-0.5 text-[10px] text-slate-400">{tag}</span>
									{/each}
									{#if item.tags.length > 4}
										<span class="text-[10px] text-slate-500">+{item.tags.length - 4}</span>
									{/if}
								</div>
							{/if}
						</div>
					</div>
				</div>

				<!-- Recent destinations -->
				{#if recentContainers.length > 0}
					<div class="space-y-1.5">
						<p class="text-xs font-medium text-slate-400 uppercase tracking-wide">Quick move</p>
						<div class="flex flex-wrap gap-2">
							{#each recentContainers as rc (rc.id)}
								<button
									class="flex items-center gap-1.5 rounded-full bg-slate-800 border border-slate-700 px-3 py-1.5 text-xs text-slate-300 active:bg-slate-700 disabled:opacity-50"
									onclick={() => moveTo(rc.id, rc.name, rc.container_path)}
									disabled={moving}
								>
									📦 {rc.name}
								</button>
							{/each}
						</div>
					</div>
				{/if}

				<!-- Quick actions -->
				<div class="grid grid-cols-2 gap-2">
					<button
						class="btn btn-secondary flex items-center justify-center gap-2"
						onclick={openMovePicker}
						disabled={moving}
					>
						<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M5 9l-3 3 3 3M9 5l3-3 3 3M15 19l-3 3-3-3M19 9l3 3-3 3M2 12h20M12 2v20"/>
						</svg>
						Move
					</button>

					<button
						class="btn btn-secondary flex items-center justify-center gap-2"
						onclick={() => goto(`/browse/item/${item.id}/edit`)}
					>
						<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/>
							<path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/>
						</svg>
						Edit
					</button>

					<button
						class="btn btn-secondary flex items-center justify-center gap-2"
						onclick={() => goto(item.is_container ? `/browse?id=${item.id}` : `/browse/item/${item.id}`)}
					>
						<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/>
							<circle cx="12" cy="12" r="3"/>
						</svg>
						View
					</button>

					<button
						class="btn btn-secondary flex items-center justify-center gap-2 text-red-400 hover:text-red-300"
						onclick={() => { confirmingDelete = true; }}
						disabled={deleting}
					>
						<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<polyline points="3 6 5 6 21 6"/>
							<path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/>
							<path d="M10 11v6M14 11v6"/>
							<path d="M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"/>
						</svg>
						Delete
					</button>
				</div>

				<button class="btn btn-ghost w-full text-sm text-slate-500" onclick={scanAnother}>
					Scan another
				</button>
			</div>
		{/if}
	</div>
</div>

<!-- ── Move picker modal ──────────────────────────────────────────────── -->
{#if showMovePicker}
<div
	class="fixed inset-0 z-50 flex flex-col bg-slate-950"
	role="dialog"
	aria-modal="true"
	aria-label="Move to container"
	tabindex="-1"
	onkeydown={(e) => e.key === 'Escape' && (showMovePicker = false)}
>
	<div class="flex items-center gap-2 border-b border-slate-800 px-3 py-2">
		<button class="btn btn-icon text-slate-400" onclick={() => (showMovePicker = false)} aria-label="Close">
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M18 6L6 18M6 6l12 12"/>
			</svg>
		</button>
		<input
			class="input flex-1"
			placeholder="Search containers…"
			bind:value={moveQuery}
			oninput={onMoveSearch}
		/>
	</div>

	{#if moveError}
		<div class="mx-4 mt-3 rounded-lg bg-red-950 border border-red-800 px-3 py-2 text-sm text-red-300">{moveError}</div>
	{/if}

	<div class="flex-1 overflow-y-auto p-3">
		{#if moveSearching}
			<div class="flex h-16 items-center justify-center">
				<div class="h-5 w-5 animate-spin rounded-full border-2 border-slate-600 border-t-indigo-500"></div>
			</div>
		{:else if moveResults.length === 0 && moveQuery}
			<p class="py-8 text-center text-sm text-slate-500">No containers found</p>
		{:else if moveResults.length === 0}
			<p class="py-8 text-center text-sm text-slate-500">Type to search containers</p>
		{:else}
			<div class="space-y-1">
				{#each moveResults as result (result.id)}
					<button
						class="flex w-full items-start gap-3 rounded-lg px-3 py-3 text-left hover:bg-slate-800 active:bg-slate-700 disabled:opacity-50"
						onclick={() => moveToFromResult(result)}
						disabled={moving}
					>
						<span class="mt-0.5 text-base">📦</span>
						<div class="min-w-0">
							<p class="text-sm font-medium text-slate-100 truncate">{result.name ?? 'Unnamed'}</p>
							{#if result.container_path}
								<p class="text-xs text-slate-500 truncate">{result.container_path}</p>
							{/if}
						</div>
						{#if moving}
							<div class="ml-auto h-4 w-4 animate-spin rounded-full border-2 border-slate-600 border-t-indigo-400 flex-shrink-0"></div>
						{/if}
					</button>
				{/each}
			</div>
		{/if}
	</div>
</div>
{/if}

<!-- ── Attach barcode to item search ─────────────────────────────────── -->
{#if showAttachSearch}
{@const attachInfo = getAttachInfo()}
<div
	class="fixed inset-0 z-50 flex flex-col bg-slate-950"
	role="dialog"
	aria-modal="true"
	aria-label="Attach barcode to item"
	tabindex="-1"
	onkeydown={(e) => e.key === 'Escape' && (showAttachSearch = false)}
>
	<div class="flex items-center gap-2 border-b border-slate-800 px-3 py-2">
		<button class="btn btn-icon text-slate-400" onclick={() => (showAttachSearch = false)} aria-label="Close">
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M18 6L6 18M6 6l12 12"/>
			</svg>
		</button>
		<input
			class="input flex-1"
			placeholder="Search items…"
			bind:value={attachQuery}
			oninput={onAttachSearch}
		/>
	</div>

	<div class="border-b border-slate-800 bg-slate-900 px-4 py-2 space-y-1.5">
		<p class="text-xs text-slate-400">
			{attachInfo?.label ?? 'Attach'}: <span class="font-mono text-slate-300">{lastBarcode}</span>
		</p>
		{#if attachInfo && !attachInfo.isAssign && !attachInfo.codeType}
			<select class="input text-xs w-full" bind:value={attachTypeOverride} aria-label="Code type">
				<option value="">Select code type…</option>
				{#each STANDARD_CODE_TYPES as t}
					<option value={t.value} title={t.description}>{t.value} — {t.description}</option>
				{/each}
			</select>
		{/if}
	</div>

	{#if attachError}
		<div class="mx-4 mt-3 rounded-lg bg-red-950 border border-red-800 px-3 py-2 text-sm text-red-300">{attachError}</div>
	{/if}

	<div class="flex-1 overflow-y-auto p-3">
		{#if attachSearching}
			<div class="flex h-16 items-center justify-center">
				<div class="h-5 w-5 animate-spin rounded-full border-2 border-slate-600 border-t-indigo-500"></div>
			</div>
		{:else if attachResults.length === 0 && attachQuery}
			<p class="py-8 text-center text-sm text-slate-500">No items found</p>
		{:else if attachResults.length === 0}
			<p class="py-8 text-center text-sm text-slate-500">Type to search items</p>
		{:else}
			<div class="space-y-1">
				{#each attachResults as result (result.id)}
					<button
						class="flex w-full items-start gap-3 rounded-lg px-3 py-3 text-left hover:bg-slate-800 active:bg-slate-700 disabled:opacity-50"
						onclick={() => attachToItem(result)}
						disabled={attaching}
					>
						<span class="mt-0.5 text-base">{result.is_container ? '📦' : '🔧'}</span>
						<div class="min-w-0 flex-1">
							<p class="text-sm font-medium text-slate-100 truncate">{result.name ?? 'Unnamed'}</p>
							{#if result.container_path}
								<p class="text-xs text-slate-500 truncate">{result.container_path}</p>
							{/if}
						</div>
						{#if attaching}
							<div class="ml-auto h-4 w-4 animate-spin rounded-full border-2 border-slate-600 border-t-indigo-400 flex-shrink-0"></div>
						{/if}
					</button>
				{/each}
			</div>
		{/if}
	</div>
</div>
{/if}

<!-- ── Delete confirm dialog ──────────────────────────────────────────── -->
{#if confirmingDelete && resolvedItem}
<div class="fixed inset-0 z-50 flex items-end bg-black/60" role="presentation" onclick={(e) => e.target === e.currentTarget && (confirmingDelete = false)}>
	<div class="w-full rounded-t-2xl bg-slate-900 p-4 pb-8 space-y-4" role="dialog" aria-modal="true">
		<div class="text-center">
			<p class="font-semibold text-slate-100">Delete "{resolvedItem.name ?? lastBarcode}"?</p>
			<p class="mt-1 text-sm text-slate-400">This can be undone from the item's history.</p>
		</div>
		<div class="flex gap-3">
			<button class="btn btn-secondary flex-1" onclick={() => (confirmingDelete = false)} disabled={deleting}>
				Cancel
			</button>
			<button class="btn flex-1 bg-red-600 text-white hover:bg-red-500 disabled:opacity-50" onclick={deleteItem} disabled={deleting}>
				{#if deleting}
					<span class="h-4 w-4 animate-spin rounded-full border-2 border-white/30 border-t-white inline-block"></span>
				{:else}
					Delete
				{/if}
			</button>
		</div>
	</div>
</div>
{/if}
