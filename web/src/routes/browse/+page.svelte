<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import { api } from '$api/client.js';
	import type { ItemSummary } from '$api/types.js';

	const ROOT_ID = '00000000-0000-0000-0000-000000000001';

	// Navigation state — current container id (null = root)
	let containerId: string = ROOT_ID;
	let breadcrumb: { id: string; name: string }[] = [];
	let children: ItemSummary[] = [];
	let loading = true;
	let error = '';

	$: containerId = $page.url.searchParams.get('id') ?? ROOT_ID;

	$: if (containerId) {
		load();
	}

	onMount(load);

	async function load() {
		loading = true;
		error = '';
		try {
			const res = await api.containers.children(containerId, { limit: 200 });
			children = res;

			if (containerId !== ROOT_ID) {
				const ancs = await api.containers.ancestors(containerId);
				breadcrumb = ancs.map((a) => ({ id: a.id, name: a.name ?? 'Container' }));
			} else {
				breadcrumb = [];
			}
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to load';
		} finally {
			loading = false;
		}
	}

	function navigate(id: string) {
		goto(`/browse?id=${id}`);
	}

	function conditionClass(condition: string | null) {
		if (!condition) return 'badge';
		return `badge badge-${condition}`;
	}
</script>

<svelte:head>
	<title>Browse — Homorg</title>
</svelte:head>

<div class="flex h-full flex-col">
	<!-- Header -->
	<header class="flex items-center gap-2 border-b border-slate-800 px-3 py-2">
		{#if containerId}
			<button class="btn btn-icon text-slate-400" on:click={() => goto('/browse')}>
				<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M15 18l-6-6 6-6" />
				</svg>
			</button>
		{/if}
		<h1 class="flex-1 text-base font-semibold text-slate-100 truncate">
			{breadcrumb.length > 0 ? breadcrumb[breadcrumb.length - 1].name : 'Browse'}
		</h1>
	</header>

	<!-- Breadcrumb -->
	{#if breadcrumb.length > 1}
		<div class="flex items-center gap-1 overflow-x-auto border-b border-slate-800 px-4 py-2 text-xs text-slate-400">
			<a href="/browse" class="flex-shrink-0 hover:text-slate-200">Root</a>
			{#each breadcrumb as crumb, i}
				<svg class="h-3 w-3 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M9 18l6-6-6-6" />
				</svg>
				{#if i < breadcrumb.length - 1}
					<a href="/browse?id={crumb.id}" class="flex-shrink-0 hover:text-slate-200 truncate max-w-24">{crumb.name}</a>
				{:else}
					<span class="flex-shrink-0 text-slate-200 truncate max-w-32">{crumb.name}</span>
				{/if}
			{/each}
		</div>
	{/if}

	<!-- Content -->
	<div class="flex-1 overflow-y-auto">
		{#if error}
			<div class="m-4 rounded-lg bg-red-950 px-4 py-3 text-sm text-red-300 border border-red-800">
				{error}
			</div>
		{:else if loading}
			<div class="flex h-32 items-center justify-center">
				<div class="h-6 w-6 animate-spin rounded-full border-2 border-slate-600 border-t-indigo-500"></div>
			</div>
		{:else if children.length === 0}
			<div class="flex h-32 flex-col items-center justify-center gap-1 text-slate-500">
				<p class="text-sm">Empty container</p>
			</div>
		{:else}
			<div class="divide-y divide-slate-800">
				{#each children as item (item.id)}
					<button
						class="flex w-full items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-slate-800/50"
						on:click={() => {
							if (item.is_container) {
								navigate(item.id);
							} else {
								goto(`/browse/item/${item.id}`);
							}
						}}
					>
						<!-- Icon -->
						<div class="flex h-10 w-10 flex-shrink-0 items-center justify-center rounded-lg text-lg {item.is_container ? 'bg-indigo-500/20 text-indigo-400' : 'bg-slate-800'}">
							{item.is_container ? '📦' : '🔧'}
						</div>

						<!-- Info -->
						<div class="min-w-0 flex-1">
							<div class="flex items-center gap-2">
								<span class="truncate font-medium text-slate-100">{item.name}</span>
								{#if item.condition && !item.is_container}
									<span class={conditionClass(item.condition)} style="font-size: 0.65rem">
										{item.condition.replace('_', ' ')}
									</span>
								{/if}
							</div>
							<div class="mt-0.5 flex items-center gap-2 text-xs text-slate-400">
								{#if item.system_barcode}
									<span class="font-mono">{item.system_barcode}</span>
								{/if}
							</div>
						</div>

						<!-- Chevron -->
						<svg class="h-4 w-4 flex-shrink-0 text-slate-600" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M9 18l6-6-6-6" />
						</svg>
					</button>
				{/each}
			</div>
		{/if}
	</div>
</div>
