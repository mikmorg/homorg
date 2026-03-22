<script lang="ts">
	import { goto } from '$app/navigation';
	import { api } from '$api/client.js';
	import type { ItemSummary } from '$api/types.js';

	let query = '';
	let results: ItemSummary[] = [];
	let loading = false;
	let searched = false;
	let debounceTimer: ReturnType<typeof setTimeout> | null = null;

	function onInput() {
		if (debounceTimer) clearTimeout(debounceTimer);
		if (!query.trim()) { results = []; searched = false; return; }
		debounceTimer = setTimeout(search, 300);
	}

	async function search() {
		loading = true;
		searched = true;
		try {
			const res = await api.search.query({ q: query, limit: 40 });
			results = res;
		} catch {
			results = [];
		} finally {
			loading = false;
		}
	}

	const CONDITION_LABELS: Record<string, string> = {
		new: 'New', like_new: 'Like new', good: 'Good',
		fair: 'Fair', poor: 'Poor', broken: 'Broken'
	};
</script>

<svelte:head>
	<title>Search — Homorg</title>
</svelte:head>

<div class="flex h-full flex-col">
	<!-- Search bar -->
	<div class="border-b border-slate-800 px-3 py-2">
		<div class="relative">
			<svg class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<circle cx="11" cy="11" r="8" />
				<path d="m21 21-4.35-4.35" />
			</svg>
			<input
				class="input pl-9"
				placeholder="Search items and containers…"
				bind:value={query}
				on:input={onInput}
			/>
		</div>
	</div>

	<!-- Results -->
	<div class="flex-1 overflow-y-auto">
		{#if loading}
			<div class="flex h-20 items-center justify-center">
				<div class="h-5 w-5 animate-spin rounded-full border-2 border-slate-600 border-t-indigo-500"></div>
			</div>
		{:else if searched && results.length === 0}
			<div class="flex h-32 flex-col items-center justify-center gap-1 text-slate-500">
				<p class="text-sm">No results for "{query}"</p>
				<p class="text-xs">Try a different search term</p>
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
					<button
						class="flex w-full items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-slate-800/50"
						on:click={() => {
							if (item.is_container) goto(`/browse?id=${item.id}`);
							else goto(`/browse/item/${item.id}`);
						}}
					>
					<div class="flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-lg text-base {item.is_container ? 'bg-indigo-500/20 text-indigo-400' : 'bg-slate-800'}">
							{item.is_container ? '📦' : '🔧'}
						</div>
						<div class="min-w-0 flex-1">
							<p class="truncate font-medium text-slate-100">{item.name}</p>
							<div class="flex items-center gap-2 mt-0.5 text-xs text-slate-400">
								{#if item.system_barcode}
									<span class="font-mono">{item.system_barcode}</span>
								{/if}
								{#if item.condition}
									<span class="badge badge-{item.condition}" style="font-size:0.6rem">
										{CONDITION_LABELS[item.condition] ?? item.condition}
									</span>
								{/if}
							</div>
						</div>
						<svg class="h-4 w-4 flex-shrink-0 text-slate-600" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M9 18l6-6-6-6" />
						</svg>
					</button>
				{/each}
			</div>
		{/if}
	</div>
</div>
