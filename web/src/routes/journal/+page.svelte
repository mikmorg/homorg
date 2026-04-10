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

	// Undo-able event types (compensating events exist for these)
	const UNDOABLE = new Set([
		'ItemCreated', 'ItemUpdated', 'ItemDeleted', 'ItemRestored',
		'ItemMoved', 'ItemImageAdded', 'ItemImageRemoved',
		'ExternalCodeAdded', 'ExternalCodeRemoved', 'QuantityAdjusted',
	]);

	// Events that are themselves compensations (don't show undo for these)
	const COMPENSATIONS = new Set(['ItemMoveReverted']);

	function isUndoable(e: StoredEvent): boolean {
		if (!UNDOABLE.has(e.event_type)) return false;
		if (COMPENSATIONS.has(e.event_type)) return false;
		// Don't offer undo if this event was already undone (has a causation_id pointing to it)
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
			// Reload to show the compensating event
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

	// Unique event types for filter dropdown
	let eventTypes = $derived([...new Set(events.map(e => e.event_type))].sort());
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
				<div class="group flex items-start gap-3 rounded-lg px-3 py-2.5 hover:bg-slate-800/60 transition-colors">
					<!-- Icon -->
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

					<!-- Content -->
					<div class="min-w-0 flex-1">
						<div class="flex items-center gap-2">
							<span class="text-sm font-medium text-slate-200">
								{eventLabel(event.event_type)}
							</span>
							{#if itemName(event)}
								<span class="truncate text-sm text-slate-400">
									{itemName(event)}
								</span>
							{/if}
						</div>

						<div class="mt-0.5 flex items-center gap-2 text-xs text-slate-500">
							<time>{timeAgo(event.created_at)}</time>
							<span>·</span>
							<span title={event.aggregate_id}>
								{event.aggregate_id.slice(0, 8)}
							</span>
							{#if event.metadata?.session_id}
								<span class="rounded bg-slate-800 px-1.5 py-0.5 text-[10px] text-slate-500">session</span>
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
