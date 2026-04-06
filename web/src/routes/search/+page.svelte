<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import { api } from '$api/client.js';
	import { toast } from '$stores/toast.js';
	import type { ItemSummary, Category, Tag, Condition } from '$api/types.js';
	import { CONDITIONS, CONDITION_LABELS } from '$api/types.js';
	import { detectBarcodeType } from '$lib/barcode-type.js';

	// IDs of results that came from a barcode resolve (shown with a badge)
	let barcodeMatchIds: Set<string> = $state(new Set());

	let query = $state('');
	let results: ItemSummary[] = $state([]);
	let loading = $state(false);
	let searched = $state(false);
	let searchError = $state('');
	let debounceTimer: ReturnType<typeof setTimeout> | null = null;
	onDestroy(() => { if (debounceTimer) clearTimeout(debounceTimer); });

	// H-9: Generation counter to discard stale search responses
	let searchGeneration = 0;

	// Filters
	let showFilters = $state(false);
	let filterCategory = $state('');
	let filterCondition: Condition | '' = $state('');
	let filterContainersOnly = $state(false);
	let filterTags: Set<string> = $state(new Set());

	// Pagination
	let cursor: string | undefined = $state(undefined);
	let hasMore = $state(false);
	let loadingMore = $state(false);

	// Taxonomy
	let categories: Category[] = $state([]);
	let tags: Tag[] = $state([]);

	let activeFilterCount = $derived(
		(filterCategory ? 1 : 0) +
		(filterCondition ? 1 : 0) +
		(filterContainersOnly ? 1 : 0) +
		filterTags.size
	);

	onMount(async () => {
		try {
			[categories, tags] = await Promise.all([
				api.categories.list(),
				api.tags.list()
			]);
		} catch { /* ignore */ }
	});

	function toggleFilterTag(name: string) {
		if (filterTags.has(name)) filterTags.delete(name);
		else filterTags.add(name);
		filterTags = new Set(filterTags);
		applyFilter();
	}

	function onInput() {
		if (debounceTimer) clearTimeout(debounceTimer);
		if (!query.trim() && !filterCategory && !filterCondition && !filterContainersOnly && filterTags.size === 0) {
			results = [];
			searched = false;
			return;
		}
		debounceTimer = setTimeout(doSearch, 300);
	}

	function looksLikeBarcode(q: string): boolean {
		const clean = q.trim();
		if (/^HOM-/i.test(clean)) return true; // system barcode prefix
		return detectBarcodeType(clean) !== '';  // known format (EAN-8/UPC/ISBN/EAN/GTIN…)
	}

	async function resolveBarcodeItems(q: string): Promise<ItemSummary[]> {
		if (!q.trim()) return [];
		try {
			const res = await api.barcodes.resolve(q.trim());
			if (res.type !== 'external' || res.item_ids.length === 0) return [];
			const items = await Promise.all(res.item_ids.map(id => api.items.get(id)));
			return items.map(item => ({
				id: item.id,
				system_barcode: item.system_barcode ?? null,
				name: item.name,
				is_container: item.is_container,
				category: item.category ?? null,
				condition: item.condition ?? null,
				container_path: item.container_path ?? null,
				parent_id: item.parent_id ?? null,
				tags: item.tags ?? [],
				is_deleted: item.is_deleted ?? false,
				created_at: item.created_at,
				updated_at: item.updated_at,
			} satisfies ItemSummary));
		} catch { return []; }
	}

	async function doSearch() {
		const gen = ++searchGeneration;
		loading = true;
		searched = true;
		searchError = '';
		cursor = undefined;
		hasMore = false;
		try {
			const [res, barcodeItems] = await Promise.all([
				api.search.query({
					q: query || undefined,
					category: filterCategory || undefined,
					condition: (filterCondition as Condition) || undefined,
					is_container: filterContainersOnly || undefined,
					tags: filterTags.size > 0 ? [...filterTags].join(',') : undefined,
					limit: 51
				}),
				looksLikeBarcode(query) ? resolveBarcodeItems(query) : Promise.resolve([] as ItemSummary[])
			]);
			if (gen !== searchGeneration) return;

			// Prepend barcode matches (deduped), track their ids for badge display
			const barcodeIds = new Set(barcodeItems.map(i => i.id));
			const deduped = barcodeItems.concat(res.filter(r => !barcodeIds.has(r.id)));
			barcodeMatchIds = barcodeIds;

			hasMore = deduped.length > 50 + barcodeItems.length
				? true
				: res.length > 50;
			const page = res.length > 50 ? res.slice(0, 50) : res;
			results = barcodeItems.concat(page.filter(r => !barcodeIds.has(r.id)));
			cursor = page.length > 0 ? page[page.length - 1].id : undefined;
		} catch (err) {
			if (gen !== searchGeneration) return;
			results = [];
			barcodeMatchIds = new Set();
			searchError = err instanceof Error ? err.message : 'Search failed';
		} finally {
			if (gen === searchGeneration) loading = false;
		}
	}

	async function loadMore() {
		if (!cursor) return;
		loadingMore = true;
		const gen = searchGeneration;
		try {
			const res = await api.search.query({
				q: query || undefined,
				category: filterCategory || undefined,
				condition: (filterCondition as Condition) || undefined,
				is_container: filterContainersOnly || undefined,
				tags: filterTags.size > 0 ? [...filterTags].join(',') : undefined,
				limit: 51,
				cursor
			});
			if (gen !== searchGeneration) return;
			hasMore = res.length > 50;
			const page = hasMore ? res.slice(0, 50) : res;
			results = [...results, ...page];
			cursor = page.length > 0 ? page[page.length - 1].id : undefined;
		} catch {
			// silent — existing results stay visible
		} finally {
			if (gen === searchGeneration) loadingMore = false;
		}
	}

	function applyFilter() {
		if (debounceTimer) clearTimeout(debounceTimer);
		doSearch();
	}

	function clearFilters() {
		filterCategory = '';
		filterCondition = '';
		filterContainersOnly = false;
		filterTags = new Set();
		barcodeMatchIds = new Set();
		applyFilter();
	}

	async function restoreItem(id: string) {
		try {
			await api.items.restore(id);
			toast('Item restored', 'success');
			doSearch();
		} catch (err) {
			toast(err instanceof Error ? err.message : 'Restore failed', 'error');
		}
	}
</script>

<svelte:head>
	<title>Search — Homorg</title>
</svelte:head>

<div class="flex h-full flex-col">
	<!-- Search bar -->
	<div class="border-b border-slate-800 px-3 py-2 space-y-2">
		<div class="relative">
			<svg class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<circle cx="11" cy="11" r="8" />
				<path d="m21 21-4.35-4.35" />
			</svg>
			<input
				class="input pl-9"
				placeholder="Search items and containers…"
				bind:value={query}
				oninput={onInput}
			/>
		</div>

		<!-- Tag chip strip -->
		{#if tags.length > 0}
		<div class="overflow-x-auto -mx-3 px-3">
			<div class="flex flex-nowrap gap-2 pb-1">
				{#each tags as tag (tag.id)}
					<button
						type="button"
						onclick={() => toggleFilterTag(tag.name)}
						class="flex-shrink-0 rounded-full px-3 py-1 text-xs font-medium transition-colors
							{filterTags.has(tag.name)
								? 'bg-indigo-600 text-white'
								: 'bg-slate-700 text-slate-300 active:bg-slate-600'}"
					>
						{tag.name}{tag.item_count ? ` (${tag.item_count})` : ''}
					</button>
				{/each}
			</div>
		</div>
		{/if}

		<!-- Filter toggle -->
		<div class="flex items-center gap-2">
			<button
				class="flex items-center gap-1 text-xs {showFilters ? 'text-indigo-400' : 'text-slate-500'} hover:text-indigo-300"
				onclick={() => { showFilters = !showFilters; }}
			>
				Filters
				{#if activeFilterCount > 0}
					<span class="rounded-full bg-indigo-600 px-1.5 py-0.5 text-[10px] leading-none text-white">{activeFilterCount}</span>
				{/if}
				{showFilters ? '▲' : '▼'}
			</button>

			{#if activeFilterCount > 0}
				<button class="text-xs text-red-400 hover:text-red-300" onclick={clearFilters}>
					Clear all
				</button>
			{/if}
		</div>

		<!-- Filters panel -->
		{#if showFilters}
			<div class="space-y-2 rounded-lg bg-slate-800/50 p-3">
				<div class="grid grid-cols-2 gap-2">
					<div>
						<label class="mb-1 block text-xs text-slate-400" for="s-cat">Category</label>
						<select id="s-cat" class="input text-sm" bind:value={filterCategory} onchange={applyFilter}>
							<option value="">Any</option>
							{#each categories as cat (cat.id)}
								<option value={cat.name}>{cat.name}{cat.item_count ? ` (${cat.item_count})` : ''}</option>
							{/each}
						</select>
					</div>
					<div>
						<label class="mb-1 block text-xs text-slate-400" for="s-cond">Condition</label>
						<select id="s-cond" class="input text-sm" bind:value={filterCondition} onchange={applyFilter}>
							<option value="">Any</option>
							{#each CONDITIONS as c}
								<option value={c}>{CONDITION_LABELS[c] ?? c}</option>
							{/each}
						</select>
					</div>
				</div>
				<div class="flex items-center gap-3">
					<label class="flex items-center gap-2 text-sm text-slate-300 cursor-pointer" for="s-containers">
						<input id="s-containers" type="checkbox" class="h-4 w-4 rounded border-slate-600 bg-slate-800" bind:checked={filterContainersOnly} onchange={applyFilter} />
						Containers only
					</label>
				</div>
			</div>
		{/if}
	</div>

	<!-- Results -->
	<div class="flex-1 overflow-y-auto">
		{#if loading}
			<div class="flex h-20 items-center justify-center">
				<div class="h-5 w-5 animate-spin rounded-full border-2 border-slate-600 border-t-indigo-500"></div>
			</div>
		{:else if searchError}
			<div class="m-4 rounded-lg bg-red-950 px-4 py-3 text-sm text-red-300 border border-red-800">{searchError}</div>
		{:else if searched && results.length === 0}
			<div class="flex h-32 flex-col items-center justify-center gap-1 text-slate-500">
				<p class="text-sm">No results</p>
				<p class="text-xs">Try different search terms or filters</p>
			</div>
		{:else if !searched}
			<div class="flex h-40 flex-col items-center justify-center gap-2 text-slate-500 px-4">
				<svg class="h-10 w-10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
					<circle cx="11" cy="11" r="8" />
					<path d="m21 21-4.35-4.35" />
				</svg>
				<p class="text-sm">Search items and containers by name</p>
			</div>
		{:else}
			<div class="divide-y divide-slate-800">
				{#each results as item (item.id)}
					<div
						class="flex w-full items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-slate-800/50 cursor-pointer"
						role="button"
						tabindex="0"
						onclick={() => {
							if (item.is_container) goto(`/browse?id=${item.id}`);
							else goto(`/browse/item/${item.id}`);
						}}
						onkeydown={(e) => { if (e.key === "Enter") { if (item.is_container) goto(`/browse?id=${item.id}`); else goto(`/browse/item/${item.id}`); } }}
					>
					<div class="flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-lg text-base {item.is_container ? 'bg-indigo-500/20 text-indigo-400' : 'bg-slate-800'}">
							{item.is_container ? '📦' : '🔧'}
						</div>
						<div class="min-w-0 flex-1">
							<p class="truncate font-medium text-slate-100">{item.name ?? 'Unnamed'}</p>
							<div class="flex items-center gap-2 mt-0.5 text-xs text-slate-400">
								{#if item.category}
									<span>{item.category}</span>
								{/if}
								{#if item.condition}
									<span class="badge badge-{item.condition}" style="font-size:0.6rem">
										{CONDITION_LABELS[item.condition] ?? item.condition}
									</span>
								{/if}
								{#if barcodeMatchIds.has(item.id)}
									<span class="rounded-full bg-amber-900/60 px-1.5 py-0.5 text-[10px] font-medium text-amber-400">barcode</span>
								{/if}
							</div>
							{#if item.container_path}
								<p class="text-xs text-slate-500 truncate mt-0.5">📍 {item.container_path}</p>
							{/if}
							{#if item.tags.length > 0}
								<div class="mt-1 flex flex-wrap gap-1">
									{#each item.tags.slice(0, 3) as tag}
										<span class="rounded-full bg-slate-700/60 px-2 py-0.5 text-[10px] text-slate-400">{tag}</span>
									{/each}
									{#if item.tags.length > 3}
										<span class="text-[10px] text-slate-500">+{item.tags.length - 3}</span>
									{/if}
								</div>
							{/if}
						</div>
						{#if item.is_deleted}
							<span class="text-xs text-emerald-400 hover:text-emerald-300 flex-shrink-0 px-2 cursor-pointer" role="button" tabindex="0"
								onclick={(e) => { e.stopPropagation(); restoreItem(item.id); }}
								onkeydown={(e) => { e.stopPropagation(); if (e.key === "Enter") restoreItem(item.id); }}>
								Restore
							</span>
						{:else}
							<svg class="h-4 w-4 flex-shrink-0 text-slate-600" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
								<path d="M9 18l6-6-6-6" />
							</svg>
						{/if}
					</div>
				{/each}
			</div>

			{#if hasMore}
				<div class="flex justify-center py-4">
					<button class="btn btn-secondary text-sm" onclick={loadMore} disabled={loadingMore}>
						{#if loadingMore}
							<span class="h-4 w-4 animate-spin rounded-full border-2 border-slate-600 border-t-indigo-400 inline-block"></span>
						{:else}
							Load more
						{/if}
					</button>
				</div>
			{/if}

			<p class="px-4 py-2 text-xs text-slate-500">{results.length} result{results.length !== 1 ? 's' : ''}{hasMore ? '+' : ''}</p>
		{/if}
	</div>
</div>
