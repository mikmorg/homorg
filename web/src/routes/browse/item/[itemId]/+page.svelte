<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { onDestroy } from 'svelte';
	import { api } from '$api/client.js';
	import type { Item, AncestorEntry, ItemSummary, StoredEvent, ContainerStats } from '$api/types.js';
	import { CONDITION_LABELS } from '$api/types.js';
	import { detectBarcodeType, STANDARD_CODE_TYPES, STANDARD_CODE_TYPE_VALUES } from '$lib/barcode-type.js';
	import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
	import CoordinateDisplay from '$lib/components/CoordinateDisplay.svelte';
	import CoordinateInput from '$lib/components/CoordinateInput.svelte';
	import LocationSchemaDisplay from '$lib/components/LocationSchemaDisplay.svelte';
	import { startCameraScanner, stopScanner, onScan } from '$lib/scanner/index.js';
	import { toast } from '$stores/toast.js';

	let itemId = $derived(page.params.itemId!);

	// Camera scan state
	let scanTarget: 'barcode' | 'external' | null = $state(null);
	let cameraContainer: HTMLDivElement | undefined = $state(undefined);
	let scanUnsub: (() => void) | null = null;
	let item: Item | null = $state(null);
	let parentItem: Item | null = $state(null);
	let ancestors: AncestorEntry[] = $state([]);
	let loading = $state(true);
	let error = $state('');

	// Delete state
	let showDeleteConfirm = $state(false);
	let deleting = $state(false);
	let actionError = $state('');

	// Convert state
	let showDownconvertConfirm = $state(false);
	let converting = $state(false);

	// Move state
	let showMovePicker = $state(false);
	let moveQuery = $state('');
	let moveResults: ItemSummary[] = $state([]);
	let moveSearching = $state(false);
	let moving = $state(false);
	let moveDebounce: ReturnType<typeof setTimeout> | null = $state(null);
	let moveTargetItem: Item | null = $state(null);
	let moveCoordinate: unknown | null = $state(null);

	onDestroy(() => {
		if (moveDebounce) clearTimeout(moveDebounce);
		closeCameraScanner();
	});

	// Container stats
	let containerStats: ContainerStats | null = $state(null);

	// History state
	let showHistory = $state(false);
	let historyEvents: StoredEvent[] = $state([]);
	let historyLoading = $state(false);

	// Quantity adjustment state
	let showQuantityAdjust = $state(false);
	let newQuantity = $state(0);
	let quantityReason = $state('');
	let adjustingQuantity = $state(false);

	// Barcode assignment state
	let showBarcodeAssign = $state(false);
	let barcodeValue = $state('');
	let assigningBarcode = $state(false);

	// External code state
	let showAddCode = $state(false);
	let newCodeType = $state('');       // '' | standard value | '__custom__' sentinel
	let newCodeTypeCustom = $state(''); // used when newCodeType === '__custom__'
	let newCodeValue = $state('');
	let addingCode = $state(false);

	async function loadItem(id: string) {
		loading = true;
		error = '';
		item = null;
		parentItem = null;
		ancestors = [];
		containerStats = null;
		// M-23: Reset UI state on navigation to prevent stale data from previous item
		showHistory = false;
		historyEvents = [];
		showMovePicker = false;
		moveResults = [];
		actionError = '';
		showDeleteConfirm = false;
		showDownconvertConfirm = false;
		showQuantityAdjust = false;
		showBarcodeAssign = false;
		showAddCode = false;
		barcodeValue = '';
		newCodeType = '';
		newCodeTypeCustom = '';
		newCodeValue = '';
		moveTargetItem = null;
		moveCoordinate = null;
		moveQuery = '';
		deleting = false;
		converting = false;
		moving = false;
		try {
			const [fetchedItem, ancs] = await Promise.all([
				api.items.get(id),
				api.containers.ancestors(id)
			]);
			// Guard: if itemId changed while we were loading, discard stale result.
			if (id !== itemId) return;
			item = fetchedItem;
			ancestors = ancs;
			if (fetchedItem.parent_id) {
				parentItem = await api.items.get(fetchedItem.parent_id);
			}
			if (fetchedItem.is_container) {
				try { containerStats = await api.containers.stats(id); } catch { /* ignore */ }
			}
		} catch (err) {
			if (id !== itemId) return;
			error = err instanceof Error ? err.message : 'Item not found';
		} finally {
			loading = false;
		}
	}

	// Reactive: re-load when itemId changes (handles same-layout navigation).
	$effect(() => { loadItem(itemId); });

	async function deleteItem() {
		deleting = true;
		actionError = '';
		try {
			await api.items.delete(itemId);
			showDeleteConfirm = false;
			toast('Item deleted', 'success');
			goto(item?.parent_id ? `/browse?id=${item.parent_id}` : '/browse');
		} catch (err) {
			actionError = err instanceof Error ? err.message : 'Delete failed';
			deleting = false;
		}
	}

	async function convertItem() {
		if (!item) return;
		converting = true;
		actionError = '';
		try {
			const wasContainer = item.is_container;
			await api.items.update(itemId, { is_container: !wasContainer });
			showDownconvertConfirm = false;
			item = await api.items.get(itemId);
			toast(wasContainer ? 'Converted to item' : 'Converted to container', 'success');
		} catch (err) {
			actionError = err instanceof Error ? err.message : 'Conversion failed';
		} finally {
			converting = false;
		}
	}

	function onConvertClick() {
		if (!item) return;
		if (item.is_container) {
			showDownconvertConfirm = true;
		} else {
			convertItem();
		}
	}

	function onMoveSearch() {
		if (moveDebounce) clearTimeout(moveDebounce);
		if (!moveQuery.trim()) { moveResults = []; return; }
		moveDebounce = setTimeout(async () => {
			moveSearching = true;
			try {
				const res = await api.search.query({ q: moveQuery, limit: 20, is_container: true });
				moveResults = res;
			} catch {
				moveResults = [];
			} finally {
				moveSearching = false;
			}
		}, 300);
	}

	async function selectMoveTarget(targetId: string) {
		try {
			const target = await api.items.get(targetId);
			if (target.location_schema) {
				moveTargetItem = target;
				moveCoordinate = null;
			} else {
				await moveToContainer(targetId);
			}
		} catch {
			await moveToContainer(targetId);
		}
	}

	async function moveToContainer(targetId: string) {
		moving = true;
		try {
			const body: { container_id: string; coordinate?: unknown } = { container_id: targetId };
			if (moveCoordinate) body.coordinate = moveCoordinate;
			await api.items.move(itemId, body);
			showMovePicker = false;
			moveTargetItem = null;
			const [newItem, newAncs] = await Promise.all([
				api.items.get(itemId),
				api.containers.ancestors(itemId)
			]);
			item = newItem;
			ancestors = newAncs;
			if (newItem.parent_id) {
				parentItem = await api.items.get(newItem.parent_id);
			}
			toast('Item moved', 'success');
		} catch (err) {
			actionError = err instanceof Error ? err.message : 'Move failed';
		} finally {
			moving = false;
		}
	}

	function formatDate(iso: string) {
		return new Date(iso).toLocaleString(undefined, {
			year: 'numeric', month: 'short', day: 'numeric',
			hour: '2-digit', minute: '2-digit'
		});
	}

	async function loadHistory() {
		if (historyEvents.length > 0) { showHistory = !showHistory; return; }
		showHistory = true;
		historyLoading = true;
		try {
			historyEvents = await api.items.history(itemId);
		} catch { /* ignore */ }
		historyLoading = false;
	}

	function eventLabel(type: string): string {
		const labels: Record<string, string> = {
			ItemCreated: 'Created',
			ItemUpdated: 'Updated',
			ItemDeleted: 'Deleted',
			ItemRestored: 'Restored',
			ItemMoved: 'Moved',
			ImageAdded: 'Image added',
			ImageRemoved: 'Image removed',
			ExternalCodeAdded: 'Code added',
			ExternalCodeRemoved: 'Code removed',
			QuantityAdjusted: 'Quantity adjusted',
			BarcodeAssigned: 'Barcode assigned',
		};
		return labels[type] ?? type.replace(/([A-Z])/g, ' $1').trim();
	}

	function shortDate(iso: string) {
		return new Date(iso).toLocaleString(undefined, {
			month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit'
		});
	}

	async function adjustQuantity() {
		adjustingQuantity = true;
		actionError = '';
		try {
			await api.items.adjustQuantity(itemId, {
				new_quantity: newQuantity,
				reason: quantityReason || undefined
			});
			item = await api.items.get(itemId);
			showQuantityAdjust = false;
			quantityReason = '';
			toast('Quantity updated', 'success');
		} catch (err) {
			actionError = err instanceof Error ? err.message : 'Adjustment failed';
		} finally {
			adjustingQuantity = false;
		}
	}

	async function assignBarcode() {
		assigningBarcode = true;
		actionError = '';
		try {
			await api.items.assignBarcode(itemId, { barcode: barcodeValue.trim() });
			item = await api.items.get(itemId);
			showBarcodeAssign = false;
			barcodeValue = '';
			toast('Barcode assigned', 'success');
		} catch (err) {
			actionError = err instanceof Error ? err.message : 'Assignment failed';
		} finally {
			assigningBarcode = false;
		}
	}

	function resolvedNewCodeType(): string {
		if (newCodeType === '__custom__') return newCodeTypeCustom.trim();
		return newCodeType;
	}

	function onNewCodeValueBlur() {
		if (!newCodeType && newCodeValue.trim()) {
			const detected = detectBarcodeType(newCodeValue.trim());
			if (detected) newCodeType = detected;
		}
	}

	async function addExternalCode() {
		addingCode = true;
		actionError = '';
		try {
			await api.items.addExternalCode(itemId, resolvedNewCodeType(), newCodeValue.trim());
			item = await api.items.get(itemId);
			showAddCode = false;
			newCodeType = '';
			newCodeTypeCustom = '';
			newCodeValue = '';
			toast('Code added', 'success');
		} catch (err) {
			actionError = err instanceof Error ? err.message : 'Failed to add code';
		} finally {
			addingCode = false;
		}
	}

	async function removeExternalCode(type: string, value: string) {
		const label = type ? `${type}: ${value}` : value;
		if (!confirm(`Remove external code ${label}?`)) return;
		actionError = '';
		try {
			await api.items.removeExternalCode(itemId, type, value);
			item = await api.items.get(itemId);
			toast('Code removed', 'success');
		} catch (err) {
			actionError = err instanceof Error ? err.message : 'Failed to remove code';
		}
	}

	async function openCameraScanner(target: 'barcode' | 'external') {
		if (target === 'barcode') showBarcodeAssign = true;
		if (target === 'external') showAddCode = true;
		scanTarget = target;
		// Wait a tick for the modal DOM to mount
		await new Promise((r) => setTimeout(r, 0));
		scanUnsub = onScan((e) => {
			if (scanTarget === 'barcode') {
				barcodeValue = e.barcode;
			} else if (scanTarget === 'external') {
				newCodeValue = e.barcode;
				const detected = detectBarcodeType(e.barcode);
				if (detected) newCodeType = detected;
			}
			closeCameraScanner();
		});
		const video = await startCameraScanner();
		if (video && cameraContainer) {
			video.style.width = '100%';
			video.style.height = '100%';
			video.style.objectFit = 'cover';
			cameraContainer.appendChild(video);
		}
	}

	function closeCameraScanner() {
		scanUnsub?.();
		scanUnsub = null;
		stopScanner();
		scanTarget = null;
	}
</script>

<svelte:window onkeydown={(e) => { if (e.key === "Escape") { if (showMovePicker) showMovePicker = false; } }} />

<svelte:head>
	<title>{item?.name ?? 'Item'} — Homorg</title>
</svelte:head>

<div class="flex h-full flex-col">
	<header class="flex items-center gap-2 border-b border-slate-800 px-3 py-2">
		<button class="btn btn-icon text-slate-400" onclick={() => goto(item?.parent_id ? `/browse?id=${item.parent_id}` : "/browse")} aria-label="Back">
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M15 18l-6-6 6-6" />
			</svg>
		</button>
		<h1 class="flex-1 text-base font-semibold text-slate-100 truncate">
			{item?.name ?? 'Loading…'}
		</h1>
		{#if item}
			<a href="/browse/item/{itemId}/edit" class="btn btn-secondary text-xs">Edit</a>
		{/if}
	</header>

	<div class="flex-1 overflow-y-auto p-4">
		{#if loading}
			<div class="flex h-32 items-center justify-center">
				<div class="h-6 w-6 animate-spin rounded-full border-2 border-slate-600 border-t-indigo-500"></div>
			</div>
		{:else if error}
			<div class="rounded-lg bg-red-950 px-4 py-3 text-sm text-red-300 border border-red-800">{error}</div>
		{:else if item}
			<div class="space-y-4">
				<!-- Name + condition -->
				<div>
					<h2 class="text-xl font-semibold text-slate-100">{item.name ?? 'Unnamed item'}</h2>
					{#if item.condition}
						<span class="badge badge-{item.condition} mt-1">{CONDITION_LABELS[item.condition] ?? item.condition}</span>
					{/if}
				</div>

				<!-- Images -->
				{#if item.images && item.images.length > 0}
					{#if item.images.length === 1}
						<img src={item.images[0].path} alt={item.images[0].caption ?? item.name ?? ''} class="w-full rounded-lg object-cover max-h-48" />
					{:else}
						<div class="flex gap-2 overflow-x-auto pb-1 -mx-4 px-4 snap-x">
							{#each item.images as img}
								<img src={img.path} alt={img.caption ?? item.name ?? ''} class="h-40 w-auto flex-shrink-0 rounded-lg object-cover snap-start" />
							{/each}
						</div>
					{/if}
				{/if}

				<!-- Location breadcrumb -->
				{#if ancestors.length > 0}
					<div class="card p-3">
						<p class="mb-1 text-xs text-slate-400 uppercase tracking-wide">Location</p>
						<div class="flex flex-wrap items-center gap-1 text-sm">
							{#each ancestors as a, i}
								<a href="/browse?id={a.id}" class="text-indigo-400 hover:underline">{a.name}</a>
								{#if i < ancestors.length - 1}
									<span class="text-slate-600">/</span>
								{/if}
							{/each}
						</div>
					</div>
				{/if}

				<!-- Position coordinate -->
				{#if item.coordinate}
					<div class="card p-3">
						<p class="mb-1 text-xs text-slate-400 uppercase tracking-wide">Position</p>
						<CoordinateDisplay coordinate={item.coordinate} schema={parentItem?.location_schema} />
					</div>
				{/if}

				<!-- Location schema (containers only) -->
				{#if item.is_container && item.location_schema}
					<div class="card p-3">
						<p class="mb-1 text-xs text-slate-400 uppercase tracking-wide">Location Schema</p>
						<LocationSchemaDisplay schema={item.location_schema} />
					</div>
				{/if}

				<!-- Container stats -->
				{#if item.is_container && containerStats}
					<div class="card p-3">
						<p class="mb-2 text-xs text-slate-400 uppercase tracking-wide">Container stats</p>
						<div class="grid grid-cols-3 gap-3 text-center">
							<div>
								<p class="text-lg font-bold text-slate-100">{containerStats.child_count}</p>
								<p class="text-xs text-slate-400">Direct</p>
							</div>
							<div>
								<p class="text-lg font-bold text-slate-100">{containerStats.descendant_count}</p>
								<p class="text-xs text-slate-400">Total</p>
							</div>
							{#if containerStats.utilization_pct !== null}
								<div>
									<p class="text-lg font-bold text-slate-100">{containerStats.utilization_pct}%</p>
									<p class="text-xs text-slate-400">Used</p>
								</div>
							{/if}
						</div>
					</div>
				{/if}

				<!-- Properties grid -->
				<div class="card divide-y divide-slate-700">
					<div class="flex items-center justify-between px-3 py-2.5">
						<span class="text-sm text-slate-400">Type</span>
						<span class="text-sm font-medium text-slate-100">{item.is_container ? 'Container' : 'Item'}</span>
					</div>
					{#if item.is_fungible}
						<div class="px-3 py-2.5">
							<div class="flex items-center justify-between">
								<span class="text-sm text-slate-400">Quantity</span>
								<div class="flex items-center gap-2">
									<span class="text-sm font-medium text-slate-100">{item.fungible_quantity ?? 0}{#if item.fungible_unit} {item.fungible_unit}{/if}</span>
									<button class="text-xs text-indigo-400 hover:text-indigo-300" onclick={() => { showQuantityAdjust = !showQuantityAdjust; if (showQuantityAdjust) newQuantity = item?.fungible_quantity ?? 0; }}>
										{showQuantityAdjust ? 'Cancel' : 'Adjust'}
									</button>
								</div>
							</div>
							{#if showQuantityAdjust}
								<div class="mt-2 flex gap-2">
									<input type="number" class="input text-sm w-24" bind:value={newQuantity} min="0" step="1" aria-label="New quantity" />
									<input type="text" class="input text-sm flex-1" bind:value={quantityReason} placeholder="Reason (optional)" aria-label="Reason for adjustment" />
									<button class="btn btn-primary text-xs px-3" onclick={adjustQuantity} disabled={adjustingQuantity}>Save</button>
								</div>
							{/if}
						</div>
					{/if}
					<div class="px-3 py-2.5">
						<div class="flex items-center justify-between">
							<span class="text-sm text-slate-400">Barcode</span>
							<div class="flex items-center gap-2">
								{#if item.system_barcode}
									<span class="text-xs font-mono text-slate-300">{item.system_barcode}</span>
								{/if}
								<button class="text-xs text-indigo-400 hover:text-indigo-300" onclick={() => { showBarcodeAssign = !showBarcodeAssign; if (showBarcodeAssign) barcodeValue = item?.system_barcode ?? ''; else if (scanTarget === 'barcode') closeCameraScanner(); }}>
									{showBarcodeAssign ? 'Cancel' : item.system_barcode ? 'Change' : 'Assign'}
								</button>
							</div>
						</div>
						{#if showBarcodeAssign}
							<div class="mt-2 flex gap-2">
								<input type="text" class="input text-sm flex-1 font-mono" bind:value={barcodeValue} placeholder="Barcode value" aria-label="Barcode value" />
								<button class="btn btn-icon text-slate-400 hover:text-indigo-400" onclick={() => openCameraScanner('barcode')} aria-label="Scan barcode with camera" title="Scan with camera">
									<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M23 19a2 2 0 01-2 2H3a2 2 0 01-2-2V8a2 2 0 012-2h4l2-3h6l2 3h4a2 2 0 012 2z"/><circle cx="12" cy="13" r="4"/></svg>
								</button>
								<button class="btn btn-primary text-xs px-3" onclick={assignBarcode} disabled={assigningBarcode || !barcodeValue.trim()}>Save</button>
							</div>
						{/if}
					</div>
					{#if item.category}
					<div class="flex items-center justify-between px-3 py-2.5">
						<span class="text-sm text-slate-400">Category</span>
						<span class="text-sm text-slate-100">{item.category}</span>
					</div>
				{/if}
					{#if item.condition}
					<div class="flex items-center justify-between px-3 py-2.5">
						<span class="text-sm text-slate-400">Condition</span>
						<span class="text-sm text-slate-100">{CONDITION_LABELS[item.condition] ?? item.condition}</span>
					</div>
				{/if}
					{#if item.acquisition_date}
					<div class="flex items-center justify-between px-3 py-2.5">
						<span class="text-sm text-slate-400">Acquired</span>
						<span class="text-xs text-slate-300">{item.acquisition_date}</span>
					</div>
				{/if}
					{#if item.acquisition_cost}
					<div class="flex items-center justify-between px-3 py-2.5">
						<span class="text-sm text-slate-400">Cost</span>
						<span class="text-sm text-slate-100">{item.currency ?? '$'}{item.acquisition_cost}</span>
					</div>
				{/if}
					{#if item.current_value}
					<div class="flex items-center justify-between px-3 py-2.5">
						<span class="text-sm text-slate-400">Value</span>
						<span class="text-sm text-slate-100">{item.currency ?? '$'}{item.current_value}</span>
					</div>
				{/if}
					{#if item.warranty_expiry}
					<div class="flex items-center justify-between px-3 py-2.5">
						<span class="text-sm text-slate-400">Warranty</span>
						<span class="text-xs text-slate-300">{item.warranty_expiry}</span>
					</div>
				{/if}
					<div class="flex items-center justify-between px-3 py-2.5">
						<span class="text-sm text-slate-400">Created</span>
						<span class="text-xs text-slate-300">{formatDate(item.created_at)}</span>
					</div>
					<div class="flex items-center justify-between px-3 py-2.5">
						<span class="text-sm text-slate-400">Updated</span>
						<span class="text-xs text-slate-300">{formatDate(item.updated_at)}</span>
					</div>
				</div>

				{#if item.description}
					<div>
						<p class="mb-1 text-xs text-slate-400 uppercase tracking-wide">Description</p>
						<p class="text-sm text-slate-300 whitespace-pre-wrap">{item.description}</p>
					</div>
				{/if}

				<!-- Tags -->
				{#if item.tags && item.tags.length > 0}
					<div>
						<p class="mb-2 text-xs text-slate-400 uppercase tracking-wide">Tags</p>
						<div class="flex flex-wrap gap-1">
							{#each item.tags as tag}
								<span class="badge">{tag}</span>
							{/each}
						</div>
					</div>
				{/if}

				<!-- External codes -->
				<div>
					<div class="flex items-center justify-between mb-2">
						<p class="text-xs text-slate-400 uppercase tracking-wide">External codes</p>
						<button class="text-xs text-indigo-400 hover:text-indigo-300" onclick={() => { showAddCode = !showAddCode; if (!showAddCode && scanTarget === 'external') closeCameraScanner(); }}>
							{showAddCode ? 'Cancel' : 'Add'}
						</button>
					</div>
					{#if showAddCode}
						<div class="mb-2 space-y-1.5">
							<div class="flex gap-2">
								<select class="input text-sm w-32 flex-shrink-0" bind:value={newCodeType} aria-label="Code type">
									<option value="">Type…</option>
									{#each STANDARD_CODE_TYPES as t}
										<option value={t.value} title={t.description}>{t.value}</option>
									{/each}
									<option disabled>──────</option>
									<option value="__custom__">Custom…</option>
								</select>
								<input type="text" class="input flex-1 font-mono text-sm" bind:value={newCodeValue} placeholder="Value" aria-label="Code value" onblur={onNewCodeValueBlur} />
								<button class="btn btn-icon text-slate-400 hover:text-indigo-400" onclick={() => openCameraScanner('external')} aria-label="Scan code with camera" title="Scan with camera">
									<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M23 19a2 2 0 01-2 2H3a2 2 0 01-2-2V8a2 2 0 012-2h4l2-3h6l2 3h4a2 2 0 012 2z"/><circle cx="12" cy="13" r="4"/></svg>
								</button>
								<button class="btn btn-primary text-xs px-3" onclick={addExternalCode}
									disabled={addingCode || !newCodeValue.trim() || !resolvedNewCodeType()}>Add</button>
							</div>
							{#if newCodeType === '__custom__'}
								<input type="text" class="input text-sm w-full" bind:value={newCodeTypeCustom} placeholder="Custom type name" aria-label="Custom code type" />
							{/if}
						</div>
					{/if}
					{#if item.external_codes && item.external_codes.length > 0}
						<div class="space-y-1">
							{#each item.external_codes as code}
								<div class="flex items-center justify-between">
									<span class="text-xs font-mono text-slate-300">{#if code.type}{code.type}: {/if}{code.value}</span>
									<button class="text-xs text-red-400 hover:text-red-300" onclick={() => removeExternalCode(code.type, code.value)}>&times;</button>
								</div>
							{/each}
						</div>
					{:else if !showAddCode}
						<p class="text-xs text-slate-500">None</p>
					{/if}
				</div>

				<!-- History -->
				<div>
					<button
						class="flex w-full items-center justify-between py-2 text-xs text-slate-400 uppercase tracking-wide"
						onclick={loadHistory}
					>
						<span>History</span>
						<svg class="h-4 w-4 transition-transform" class:rotate-180={showHistory} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M6 9l6 6 6-6" />
						</svg>
					</button>
					{#if showHistory}
						{#if historyLoading}
							<div class="flex justify-center py-4">
								<div class="h-5 w-5 animate-spin rounded-full border-2 border-slate-600 border-t-indigo-500"></div>
							</div>
						{:else if historyEvents.length === 0}
							<p class="py-2 text-xs text-slate-500">No history available</p>
						{:else}
							<div class="space-y-1.5 pb-2">
								{#each historyEvents as evt}
									<div class="flex items-center justify-between rounded bg-slate-800/50 px-3 py-1.5">
										<span class="text-xs text-slate-300">{eventLabel(evt.event_type)}</span>
										<span class="text-xs text-slate-500">{shortDate(evt.created_at)}</span>
									</div>
								{/each}
							</div>
						{/if}
					{/if}
				</div>

				<!-- Actions -->
				<div class="space-y-2 pt-2">
					{#if !item.is_fungible}
						<button class="btn btn-secondary w-full" onclick={onConvertClick} disabled={converting}>
							{#if converting}
								Converting…
							{:else if item.is_container}
								Convert to item
							{:else}
								Convert to container
							{/if}
						</button>
					{/if}
					<button class="btn btn-secondary w-full" onclick={() => { showMovePicker = true; moveQuery = ''; moveResults = []; moveTargetItem = null; moveCoordinate = null; }}>
						Move to another container
					</button>
					<button class="btn btn-danger w-full" onclick={() => (showDeleteConfirm = true)} disabled={deleting}>
						Delete item
					</button>
				</div>

				{#if actionError}
					<p class="text-sm text-red-400">{actionError}</p>
				{/if}
			</div>
		{/if}
	</div>
</div>

<!-- Camera scanner overlay -->
{#if scanTarget}
<div class="fixed inset-0 z-50 flex flex-col bg-slate-950">
	<div class="flex items-center justify-between border-b border-slate-800 px-3 py-2">
		<span class="text-sm font-medium text-slate-200">
			{scanTarget === 'barcode' ? 'Scan system barcode' : 'Scan external code'}
		</span>
		<button class="btn btn-icon text-slate-400" onclick={closeCameraScanner} aria-label="Close camera">
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M18 6L6 18M6 6l12 12" />
			</svg>
		</button>
	</div>
	<div class="flex flex-1 items-center justify-center" bind:this={cameraContainer}>
		<!-- video element is appended here by startCameraScanner -->
	</div>
</div>
{/if}

<!-- Delete confirmation -->
<ConfirmDialog
	bind:open={showDeleteConfirm}
	title="Delete {item?.name ?? 'item'}?"
	message="This action can be undone from the event log."
	confirmLabel={deleting ? 'Deleting…' : 'Delete'}
	destructive={true}
	loading={deleting}
	onConfirm={deleteItem}
/>

<!-- Downconvert confirmation -->
<ConfirmDialog
	bind:open={showDownconvertConfirm}
	title="Convert to item?"
	message="This will remove the container status from {item?.name ?? 'this item'}. The container must be empty."
	confirmLabel={converting ? 'Converting…' : 'Convert'}
	destructive={false}
	loading={converting}
	onConfirm={convertItem}
/>

<!-- Move picker -->
{#if showMovePicker}
<div class="fixed inset-0 z-50 flex flex-col bg-slate-950">
	<div class="flex items-center gap-2 border-b border-slate-800 px-3 py-2">
		<button class="btn btn-icon text-slate-400" onclick={() => { showMovePicker = false; moveTargetItem = null; }} aria-label="Close">
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M18 6L6 18M6 6l12 12" />
			</svg>
		</button>
		<input
			class="input flex-1"
			placeholder="Search containers…"
			bind:value={moveQuery}
			oninput={onMoveSearch}
		/>
	</div>

	<div class="flex-1 overflow-y-auto">
		{#if moveTargetItem}
			<!-- Coordinate step -->
			<div class="p-4 space-y-4">
				<div class="card p-3">
					<p class="text-xs text-slate-400 mb-1">Moving to</p>
					<p class="font-medium text-slate-100">{moveTargetItem.name}</p>
				</div>
				<CoordinateInput schema={moveTargetItem.location_schema} bind:value={moveCoordinate} />
				<div class="flex gap-2">
					<button class="btn btn-secondary flex-1" onclick={() => (moveTargetItem = null)}>Back</button>
					<button class="btn btn-primary flex-1" onclick={() => moveToContainer(moveTargetItem?.id ?? '')} disabled={moving}>
						{moving ? 'Moving…' : 'Move here'}
					</button>
				</div>
			</div>
		{:else if moveSearching}
			<div class="flex h-20 items-center justify-center">
				<div class="h-5 w-5 animate-spin rounded-full border-2 border-slate-600 border-t-indigo-500"></div>
			</div>
		{:else if moveQuery && moveResults.length === 0}
			<div class="flex h-20 items-center justify-center text-sm text-slate-500">
				No containers found
			</div>
		{:else if !moveQuery}
			<div class="flex h-20 items-center justify-center text-sm text-slate-500">
				Search for a destination container
			</div>
		{:else}
			<div class="divide-y divide-slate-800">
				{#each moveResults as container (container.id)}
					<button
						class="flex w-full items-center gap-3 px-4 py-3 text-left hover:bg-slate-800/50"
						onclick={() => selectMoveTarget(container.id)}
						disabled={moving}
					>
						<div class="flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-lg bg-indigo-500/20 text-indigo-400">
							📦
						</div>
						<div class="min-w-0 flex-1">
							<p class="truncate font-medium text-slate-100">{container.name}</p>
							{#if container.system_barcode}
								<p class="text-xs text-slate-400 font-mono">{container.system_barcode}</p>
							{/if}
						</div>
					</button>
				{/each}
			</div>
		{/if}
	</div>
</div>
{/if}
