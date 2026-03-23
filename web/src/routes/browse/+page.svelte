<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import { api } from '$api/client.js';
	import type { Item, ItemSummary, CreateItemRequest, Condition } from '$api/types.js';
	import { CONDITIONS } from '$api/types.js';
	import CoordinateInput from '$lib/components/CoordinateInput.svelte';

	const ROOT_ID = '00000000-0000-0000-0000-000000000001';

	// Navigation state — current container id (null = root)
	let containerId: string = ROOT_ID;
	let breadcrumb: { id: string; name: string }[] = [];
	let children: ItemSummary[] = [];
	let containerItem: Item | null = null;
	let loading = true;
	let error = '';

	// Create form state
	let showCreate = false;
	let createType: 'item' | 'container' = 'item';
	let createName = '';
	let createDescription = '';
	let createCondition: Condition | '' = '';
	let createCoordinate: unknown | null = null;
	let creating = false;
	let createError = '';

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
				const [ancs, item] = await Promise.all([
					api.containers.ancestors(containerId),
					api.items.get(containerId)
				]);
				breadcrumb = ancs.map((a) => ({ id: a.id, name: a.name ?? 'Container' }));
				containerItem = item;
			} else {
				breadcrumb = [];
				containerItem = null;
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

	function openCreate(type: 'item' | 'container') {
		createType = type;
		createName = '';
		createDescription = '';
		createCondition = '';
		createCoordinate = null;
		createError = '';
		showCreate = true;
	}

	async function submitCreate() {
		if (!createName.trim()) { createError = 'Name is required'; return; }
		creating = true;
		createError = '';
		try {
			const body: CreateItemRequest = {
				parent_id: containerId,
				name: createName.trim(),
				is_container: createType === 'container'
			};
			if (createDescription.trim()) body.description = createDescription.trim();
			if (createCondition && createType === 'item') body.condition = createCondition;
			if (createCoordinate) body.coordinate = createCoordinate;
			await api.items.create(body);
			showCreate = false;
			await load();
		} catch (err) {
			createError = err instanceof Error ? err.message : 'Failed to create';
		} finally {
			creating = false;
		}
	}

	function conditionClass(condition: string | null) {
		if (!condition) return 'badge';
		return `badge badge-${condition}`;
	}

	const CONDITION_LABELS: Record<string, string> = {
		new: 'New', like_new: 'Like new', good: 'Good',
		fair: 'Fair', poor: 'Poor', broken: 'Broken'
	};
</script>

<svelte:head>
	<title>Browse — Homorg</title>
</svelte:head>

<div class="flex h-full flex-col">
	<!-- Header -->
	<header class="flex items-center gap-2 border-b border-slate-800 px-3 py-2">
		{#if containerId !== ROOT_ID}
			<button class="btn btn-icon text-slate-400" on:click={() => { const parent = breadcrumb.length > 1 ? breadcrumb[breadcrumb.length - 2].id : ROOT_ID; goto(`/browse?id=${parent}`); }} aria-label="Back">
				<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M15 18l-6-6 6-6" />
				</svg>
			</button>
		{/if}
		<h1 class="flex-1 text-base font-semibold text-slate-100 truncate">
			{breadcrumb.length > 0 ? breadcrumb[breadcrumb.length - 1].name : 'Browse'}
		</h1>
		<button class="btn btn-icon text-indigo-400" on:click={() => openCreate('container')} aria-label="New container">
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<rect x="2" y="7" width="20" height="14" rx="2" />
				<path d="M12 11v6M9 14h6" />
			</svg>
		</button>
		<button class="btn btn-icon text-indigo-400" on:click={() => openCreate('item')} aria-label="New item">
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<line x1="12" y1="5" x2="12" y2="19" />
				<line x1="5" y1="12" x2="19" y2="12" />
			</svg>
		</button>
	</header>

	<!-- Breadcrumb -->
	{#if breadcrumb.length > 1}
		<div class="flex items-center gap-1 overflow-x-auto border-b border-slate-800 px-4 py-2 text-xs text-slate-400">
			<a href="/browse?id={ROOT_ID}" class="flex-shrink-0 hover:text-slate-200">Root</a>
			{#each breadcrumb.slice(1) as crumb, i}
				<svg class="h-3 w-3 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M9 18l6-6-6-6" />
				</svg>
				{#if i < breadcrumb.length - 2}
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
			<div class="flex h-48 flex-col items-center justify-center gap-3 text-slate-500 px-4">
				<svg class="h-12 w-12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
					<rect x="2" y="7" width="20" height="14" rx="2" />
					<path d="M16 7V5a2 2 0 0 0-2-2h-4a2 2 0 0 0-2 2v2" />
				</svg>
				<p class="text-sm">This container is empty</p>
				<div class="flex gap-2">
					<button class="btn btn-secondary text-xs" on:click={() => openCreate('container')}>Add container</button>
					<button class="btn btn-primary text-xs" on:click={() => openCreate('item')}>Add item</button>
				</div>
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

<!-- Create form bottom sheet -->
{#if showCreate}
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="fixed inset-0 z-50 flex flex-col justify-end bg-black/60" on:click|self={() => (showCreate = false)} on:keydown={(e) => e.key === 'Escape' && (showCreate = false)}>
	<div class="rounded-t-2xl bg-slate-900 p-4 pb-8">
		<div class="mb-4 flex items-center justify-between">
			<h2 class="text-base font-semibold text-slate-100">
				New {createType === 'container' ? 'container' : 'item'}
			</h2>
			<button class="btn btn-icon text-slate-400" on:click={() => (showCreate = false)} aria-label="Close">
				<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M18 6L6 18M6 6l12 12" />
				</svg>
			</button>
		</div>

		{#if createError}
			<div class="mb-3 rounded-lg bg-red-950 px-3 py-2 text-sm text-red-300 border border-red-800">{createError}</div>
		{/if}

		<div class="space-y-3">
			<div>
				<label class="mb-1 block text-sm font-medium text-slate-300" for="create-name">Name</label>
				<input
					id="create-name"
					class="input"
					bind:value={createName}
					placeholder={createType === 'container' ? 'e.g. Garage shelf' : 'e.g. Cordless drill'}
					on:keydown={(e) => e.key === 'Enter' && submitCreate()}
				/>
			</div>

			<div>
				<label class="mb-1 block text-sm font-medium text-slate-300" for="create-desc">Description</label>
				<textarea id="create-desc" class="input min-h-16 resize-y" bind:value={createDescription} placeholder="Optional" rows="2"></textarea>
			</div>

			{#if createType === 'item'}
				<div>
					<label class="mb-1 block text-sm font-medium text-slate-300" for="create-condition">Condition</label>
					<select id="create-condition" class="input" bind:value={createCondition}>
						<option value="">Not set</option>
						{#each CONDITIONS as c}
							<option value={c}>{CONDITION_LABELS[c] ?? c}</option>
						{/each}
					</select>
				</div>
			{/if}

			{#if containerItem?.location_schema}
				<CoordinateInput schema={containerItem.location_schema} bind:value={createCoordinate} />
			{/if}

			<button class="btn btn-primary w-full" on:click={submitCreate} disabled={creating}>
				{creating ? 'Creating…' : `Create ${createType}`}
			</button>
		</div>
	</div>
</div>
{/if}
