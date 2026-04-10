<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '$api/client.js';
	import { toast } from '$stores/toast.js';
	import type { StoredEvent } from '$api/types.js';

	let events: StoredEvent[] = $state([]);
	let loading = $state(true);
	let loadingMore = $state(false);
	let hasMore = $state(true);
	let undoingId: string | null = $state(null);
	let filterType = $state('');
	let lightboxUrl: string | null = $state(null);

	const PAGE_SIZE = 50;

	const EVENT_LABELS: Record<string, string> = {
		ItemCreated: 'Created',
		ItemUpdated: 'Updated',
		ItemDeleted: 'Deleted',
		ItemRestored: 'Restored',
		ItemMoved: 'Moved',
		ItemMoveReverted: 'Move undone',
		ItemImageAdded: 'Photo added',
		ItemImageRemoved: 'Photo removed',
		ExternalCodeAdded: 'Code added',
		ExternalCodeRemoved: 'Code removed',
		QuantityAdjusted: 'Qty adjusted',
		BarcodeGenerated: 'Barcode generated',
		ItemBarcodeAssigned: 'Barcode assigned',
	};

	const EVENT_ICONS: Record<string, string> = {
		ItemCreated: '+',
		ItemUpdated: '~',
		ItemDeleted: '×',
		ItemRestored: '↩',
		ItemMoved: '→',
		ItemMoveReverted: '←',
		ItemImageAdded: '📷',
		ItemImageRemoved: '🗑',
		ExternalCodeAdded: '🏷',
		ExternalCodeRemoved: '🏷',
		QuantityAdjusted: '#',
		BarcodeGenerated: '⊞',
		ItemBarcodeAssigned: '⊞',
	};

	const UNDOABLE = new Set([
		'ItemCreated', 'ItemUpdated', 'ItemDeleted', 'ItemRestored',
		'ItemMoved', 'ItemImageAdded', 'ItemImageRemoved',
		'ExternalCodeAdded', 'ExternalCodeRemoved', 'QuantityAdjusted',
	]);

	const COMPENSATIONS = new Set(['ItemMoveReverted']);

	function isUndoable(e: StoredEvent): boolean {
		if (!UNDOABLE.has(e.event_type)) return false;
		if (COMPENSATIONS.has(e.event_type)) return false;
		if (e.metadata?.causation_id) return false;
		return true;
	}

	function eventLabel(type: string): string {
		return EVENT_LABELS[type] ?? type.replace(/([A-Z])/g, ' $1').trim();
	}

	function eventIcon(type: string): string {
		return EVENT_ICONS[type] ?? '•';
	}

	function itemName(e: StoredEvent): string {
		const data = e.event_data as Record<string, unknown>;
		return (data?.name as string) ?? (data?.item_name as string) ?? '';
	}

	function imageUrl(e: StoredEvent): string | null {
		const data = e.event_data as Record<string, unknown>;
		return (data?.url as string) ?? (data?.image_url as string) ?? null;
	}

	function timeAgo(iso: string): string {
		const diff = Date.now() - new Date(iso).getTime();
		const mins = Math.floor(diff / 60000);
		if (mins < 1) return 'just now';
		if (mins < 60) return `${mins}m ago`;
		const hrs = Math.floor(mins / 60);
		if (hrs < 24) return `${hrs}h ago`;
		const days = Math.floor(hrs / 24);
		return `${days}d ago`;
	}

	function fullTime(iso: string): string {
		return new Date(iso).toLocaleString();
	}

	async function loadEvents(append = false) {
		if (append) loadingMore = true; else loading = true;
		try {
			const beforeId = append && events.length > 0 ? events[events.length - 1].id : undefined;
			const result = await api.events.list({
				limit: PAGE_SIZE,
				before_id: beforeId,
				event_type: filterType || undefined,
			});
			if (append) {
				events = [...events, ...result];
			} else {
				events = result;
			}
			hasMore = result.length === PAGE_SIZE;
		} catch (err) {
			toast(err instanceof Error ? err.message : 'Failed to load events');
		} finally {
			loading = false;
			loadingMore = false;
		}
	}

	async function undoEvent(eventId: string) {
		undoingId = eventId;
		try {
			await api.undo.single(eventId);
			toast('Event undone');
			await loadEvents();
		} catch (err) {
			toast(err instanceof Error ? err.message : 'Undo failed');
		} finally {
			undoingId = null;
		}
	}

	function applyFilter() {
		loadEvents();
	}

	onMount(() => loadEvents());
</script>

<svelte:head><title>Journal — Homorg</title></svelte:head>

<div class="mx-auto max-w-3xl px-4 py-6">
	<div class="mb-6 flex items-center justify-between">
		<h1 class="text-2xl font-bold text-slate-100">Journal</h1>

		<select
			class="rounded-lg border border-slate-700 bg-slate-800 px-3 py-1.5 text-sm text-slate-300"
			bind:value={filterType}
			onchange={applyFilter}
		>
			<option value="">All events</option>
			{#each Object.keys(EVENT_LABELS) as type}
				<option value={type}>{EVENT_LABELS[type]}</option>
			{/each}
		</select>
	</div>

	{#if loading}
		<div class="flex justify-center py-12">
			<div class="h-8 w-8 animate-spin rounded-full border-2 border-slate-600 border-t-emerald-400"></div>
		</div>
	{:else if events.length === 0}
		<p class="py-12 text-center text-slate-500">No events found</p>
	{:else}
		<div class="space-y-1">
			{#each events as event (event.id)}
				{@const data = event.event_data as Record<string, unknown>}
				{@const imgUrl = imageUrl(event)}
				<div class="group flex items-start gap-3 rounded-lg px-3 py-2.5 hover:bg-slate-800/60 transition-colors">
					<!-- Icon / Image thumbnail -->
					{#if imgUrl}
						<button class="mt-0.5 flex-shrink-0 cursor-zoom-in" onclick={() => lightboxUrl = imgUrl}>
							<img src={imgUrl} alt="" class="h-9 w-9 rounded object-cover border border-slate-700 hover:border-emerald-500 transition-colors" />
						</button>
					{:else}
						<span class="mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-slate-800 text-sm
							{event.event_type === 'ItemDeleted' ? 'text-red-400' : ''}
							{event.event_type === 'ItemCreated' ? 'text-emerald-400' : ''}
							{event.event_type === 'ItemMoved' ? 'text-blue-400' : ''}
							{event.event_type === 'ItemUpdated' ? 'text-amber-400' : ''}
							{event.event_type === 'ItemRestored' ? 'text-green-400' : ''}
							{event.event_type === 'ItemImageAdded' ? 'text-purple-400' : ''}
						">
							{eventIcon(event.event_type)}
						</span>
					{/if}

					<!-- Content -->
					<div class="min-w-0 flex-1">
						<div class="flex items-center gap-2">
							<span class="text-sm font-medium text-slate-200">
								{eventLabel(event.event_type)}
							</span>
							{#if itemName(event)}
								<a href="/browse/item/{event.aggregate_id}" class="truncate text-sm text-slate-400 hover:text-emerald-400 transition-colors">
									{itemName(event)}
								</a>
							{/if}
						</div>

						<div class="mt-0.5 flex flex-wrap items-center gap-2 text-xs text-slate-500">
							<time title={fullTime(event.created_at)}>{timeAgo(event.created_at)}</time>
							<span>·</span>
							<a href="/browse/item/{event.aggregate_id}" class="hover:text-emerald-400 transition-colors" title={event.aggregate_id}>
								{event.aggregate_id.slice(0, 8)}
							</a>
							{#if event.metadata?.session_id}
								<a href="/stocker/{event.metadata.session_id}" class="rounded bg-slate-800 px-1.5 py-0.5 text-[10px] text-slate-400 hover:text-emerald-400 transition-colors">
									session
								</a>
							{/if}
							{#if event.metadata?.causation_id}
								<span class="rounded bg-amber-900/40 px-1.5 py-0.5 text-[10px] text-amber-400">undo</span>
							{/if}
						</div>
					</div>

					<!-- Undo button -->
					{#if isUndoable(event)}
						<button
							class="shrink-0 rounded px-2 py-1 text-xs text-slate-500 opacity-0 transition-opacity hover:bg-slate-700 hover:text-slate-300 group-hover:opacity-100"
							onclick={() => undoEvent(event.event_id)}
							disabled={undoingId === event.event_id}
						>
							{undoingId === event.event_id ? '…' : 'Undo'}
						</button>
					{/if}
				</div>
			{/each}
		</div>

		{#if hasMore}
			<div class="mt-4 flex justify-center">
				<button
					class="rounded-lg bg-slate-800 px-4 py-2 text-sm text-slate-300 hover:bg-slate-700"
					onclick={() => loadEvents(true)}
					disabled={loadingMore}
				>
					{loadingMore ? 'Loading…' : 'Load more'}
				</button>
			</div>
		{/if}
	{/if}
</div>

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
