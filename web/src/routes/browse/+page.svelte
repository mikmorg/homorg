<script lang="ts">
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { onDestroy } from 'svelte';
	import { api } from '$api/client.js';
	import type { Item, ItemSummary, CreateItemRequest, Condition, Category, Tag, ContainerType } from '$api/types.js';
	import { CONDITIONS, CONDITION_LABELS, conditionClass } from '$api/types.js';
	import CoordinateInput from '$lib/components/CoordinateInput.svelte';
	import LocationSchemaEditor from '$lib/components/LocationSchemaEditor.svelte';
	import { toast } from '$stores/toast.js';

	const ROOT_ID = '00000000-0000-0000-0000-000000000001';

	// Navigation state — current container id (null = root)
	let containerId: string = $derived(page.url.searchParams.get('id') ?? ROOT_ID);
	let breadcrumb: { id: string; name: string }[] = $state([]);
	let children: ItemSummary[] = $state([]);
	let containerItem: Item | null = $state(null);
	let loading = $state(true);
	let error = $state('');

	// Create form state
	let showCreate = $state(false);
	let createType: 'item' | 'container' = $state('item');
	let createName = $state('');
	let createDescription = $state('');
	let createCondition: Condition | '' = $state('');
	let createCoordinate: unknown | null = $state(null);
	let createCategoryId: string | null = $state(null);
	let createSelectedTagIds: Set<string> = $state(new Set());
	let createContainerTypeId: string | null = $state(null);
	let createLocationSchema: unknown | null = $state(null);
	let createImages: File[] = $state([]);
	let createImagePreviews: string[] = $state([]);
	let createIsFungible = $state(false);
	let createFungibleUnit = $state('');
	let createFungibleQty = $state('');
	let createAcqCost = $state('');
	let createCurrency = $state('');
	let showCreateAdvanced = $state(false);
	let creating = $state(false);
	let createError = $state('');

	// Taxonomy data (loaded once for the create form)
	let categories: Category[] = $state([]);
	let allTags: Tag[] = $state([]);
	let containerTypes: ContainerType[] = $state([]);
	let taxonomyLoaded = $state(false);

	// Sort state
	let sortBy: 'name' | 'created_at' | 'category' = $state('name');
	let sortDir: 'asc' | 'desc' = $state('asc');

	// Pagination
	const PAGE_SIZE = 50;
	let cursor: string | undefined = $state(undefined);
	let hasMore = $state(false);
	let loadingMore = $state(false);

	$effect(() => {
		if (containerId) {
			load(containerId);
		}
	});

	async function load(targetId?: string) {
		const id = targetId ?? containerId;
		loading = true;
		error = '';
		cursor = undefined;
		hasMore = false;
		try {
			const res = await api.containers.children(id, { limit: PAGE_SIZE + 1, sort_by: sortBy, sort_dir: sortDir });
			// H-8: Guard against stale responses from rapid navigation
			if (id !== containerId) return;
			hasMore = res.length > PAGE_SIZE;
			children = hasMore ? res.slice(0, PAGE_SIZE) : res;
			cursor = children.length > 0 ? children[children.length - 1].id : undefined;

			if (id !== ROOT_ID) {
				const [ancs, item] = await Promise.all([
					api.containers.ancestors(id),
					api.items.get(id)
				]);
				if (id !== containerId) return;
				breadcrumb = [
					...ancs.map((a) => ({ id: a.id, name: a.name ?? 'Container' })),
					{ id: id, name: item?.name ?? 'Container' }
				];
				containerItem = item;
			} else {
				breadcrumb = [];
				containerItem = null;
			}
		} catch (err) {
			if (id !== containerId) return;
			error = err instanceof Error ? err.message : 'Failed to load';
		} finally {
			loading = false;
		}
	}

	async function loadMore() {
		if (!cursor || loadingMore || !hasMore) return;
		loadingMore = true;
		try {
			const res = await api.containers.children(containerId, {
				limit: PAGE_SIZE + 1,
				cursor,
				sort_by: sortBy,
				sort_dir: sortDir
			});
			hasMore = res.length > PAGE_SIZE;
			const pageSlice = hasMore ? res.slice(0, PAGE_SIZE) : res;
			children = [...children, ...pageSlice];
			cursor = children.length > 0 ? children[children.length - 1].id : undefined;
		} catch (err) {
			toast(err instanceof Error ? err.message : 'Failed to load more', 'error');
		} finally {
			loadingMore = false;
		}
	}

	function navigate(id: string) {
		goto(`/browse?id=${id}`);
	}

	async function loadTaxonomy() {
		if (taxonomyLoaded) return;
		try {
			const [cats, tags, ctypes] = await Promise.all([
				api.categories.list(),
				api.tags.list(),
				api.containerTypes.list()
			]);
			categories = cats;
			allTags = tags;
			containerTypes = ctypes;
			taxonomyLoaded = true;
		} catch {
			// Non-critical — form works without taxonomy
		}
	}

	function revokeCreatePreviews() {
		createImagePreviews.forEach((url) => URL.revokeObjectURL(url));
		createImagePreviews = [];
		createImages = [];
	}

	function closeCreate() {
		revokeCreatePreviews();
		showCreate = false;
	}

	onDestroy(revokeCreatePreviews);

	function openCreate(type: 'item' | 'container') {
		createType = type;
		createName = '';
		createDescription = '';
		createCondition = '';
		createCoordinate = null;
		createCategoryId = null;
		createSelectedTagIds = new Set();
		createContainerTypeId = null;
		createLocationSchema = null;
		revokeCreatePreviews();
		createIsFungible = false;
		createFungibleUnit = '';
		createFungibleQty = '';
		createAcqCost = '';
		createCurrency = '';
		showCreateAdvanced = false;
		createError = '';
		showCreate = true;
		loadTaxonomy();
	}

	function handleImageAdd(e: Event) {
		const input = e.target as HTMLInputElement;
		const files = input.files;
		if (!files) return;
		for (let i = 0; i < files.length; i++) {
			createImages = [...createImages, files[i]];
			createImagePreviews = [...createImagePreviews, URL.createObjectURL(files[i])];
		}
		input.value = '';
	}

	function removeCreateImage(idx: number) {
		URL.revokeObjectURL(createImagePreviews[idx]);
		createImages = createImages.filter((_, i) => i !== idx);
		createImagePreviews = createImagePreviews.filter((_, i) => i !== idx);
	}

	function toggleCreateTag(tagId: string) {
		if (createSelectedTagIds.has(tagId)) {
			createSelectedTagIds.delete(tagId);
		} else {
			createSelectedTagIds.add(tagId);
		}
		createSelectedTagIds = new Set(createSelectedTagIds);
	}

	function applyContainerType() {
		if (!createContainerTypeId) {
			createLocationSchema = null;
			return;
		}
		const ct = containerTypes.find((t) => t.id === createContainerTypeId);
		if (ct) {
			createLocationSchema = ct.default_location_schema ?? null;
		}
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
			if (createContainerTypeId) body.container_type_id = createContainerTypeId;
			if (createIsFungible && createType === 'item') {
				body.is_fungible = true;
				if (createFungibleUnit.trim()) body.fungible_unit = createFungibleUnit.trim();
				const qty = parseInt(createFungibleQty);
				if (Number.isFinite(qty)) body.fungible_quantity = qty;
			}
			// B5: send as string to preserve decimal precision.
			if (createAcqCost.trim() && Number.isFinite(parseFloat(createAcqCost))) {
				body.acquisition_cost = createAcqCost.trim();
			}
			if (createCurrency.trim()) body.currency = createCurrency.trim();

			// Category
			if (createCategoryId) {
				const cat = categories.find((c) => c.id === createCategoryId);
				if (cat) body.category = cat.name;
			}

			// Tags
			const tagNames = allTags
				.filter((t) => createSelectedTagIds.has(t.id))
				.map((t) => t.name);
			if (tagNames.length > 0) body.tags = tagNames;

			const event = await api.items.create(body);

			// Upload images after creation
			const newItemId = event.aggregate_id;
			let postCreateWarnings: string[] = [];
			for (const file of createImages) {
				try {
					await api.items.uploadImage(newItemId, file);
				} catch {
					postCreateWarnings.push(`Image "${file.name}" failed to upload`);
				}
			}

			// Update container schema if set
			if (createType === 'container' && createLocationSchema) {
				try {
					await api.containers.updateSchema(newItemId, createLocationSchema);
				} catch {
					postCreateWarnings.push('Location schema failed to save');
				}
			}

			closeCreate();
			createIsFungible = false;
			createFungibleUnit = '';
			createFungibleQty = '';
			createAcqCost = '';
			createCurrency = '';
			showCreateAdvanced = false;
			// Show success first so the user knows the item exists, then any partial failures
			toast(createType === 'container' ? 'Container created' : 'Item created', 'success');
			if (postCreateWarnings.length > 0) {
				for (const w of postCreateWarnings) toast(w, 'error');
			}
			await load();
		} catch (err) {
			createError = err instanceof Error ? err.message : 'Failed to create';
		} finally {
			creating = false;
		}
	}
</script>

<svelte:window onkeydown={(e) => { if (e.key === "Escape" && showCreate) closeCreate(); }} />

<svelte:head>
	<title>Browse — Homorg</title>
</svelte:head>

<div class="flex h-full flex-col">
	<!-- Header -->
	<header class="flex items-center gap-2 border-b border-slate-800 px-3 py-2">
		{#if containerId !== ROOT_ID}
			<button class="btn btn-icon text-slate-400" onclick={() => { const parent = breadcrumb.length > 1 ? breadcrumb[breadcrumb.length - 2].id : ROOT_ID; goto(`/browse?id=${parent}`); }} aria-label="Back">
				<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M15 18l-6-6 6-6" />
				</svg>
			</button>
		{/if}
		<h1 class="flex-1 text-base font-semibold text-slate-100 truncate">
			{#if containerId !== ROOT_ID}
				<a href="/browse/item/{containerId}" class="hover:text-indigo-300 transition-colors">{breadcrumb.length > 0 ? breadcrumb[breadcrumb.length - 1].name : 'Container'}</a>
			{:else}
				Browse
			{/if}
		</h1>
		{#if containerId !== ROOT_ID}
			<a href="/browse/item/{containerId}/edit" class="btn btn-icon text-slate-400" aria-label="Edit container">
				<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M17 3a2.85 2.85 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z" />
					<path d="m15 5 4 4" />
				</svg>
			</a>
		{/if}
		<button class="btn btn-icon text-indigo-400" onclick={() => openCreate('container')} aria-label="New container">
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<rect x="2" y="7" width="20" height="14" rx="2" />
				<path d="M12 11v6M9 14h6" />
			</svg>
		</button>
		<button class="btn btn-icon text-indigo-400" onclick={() => openCreate('item')} aria-label="New item">
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

	<!-- Sort bar -->
	{#if children.length > 1}
		<div class="flex items-center gap-2 border-b border-slate-800 px-3 py-1.5">
			<span class="text-xs text-slate-500">Sort:</span>
			<select class="bg-transparent text-xs text-slate-300 border-0 py-0 pr-6 pl-0 focus:ring-0" bind:value={sortBy} onchange={() => load()}>
				<option value="name">Name</option>
				<option value="created_at">Created</option>
				<option value="category">Category</option>
			</select>
			<button class="text-xs text-slate-400 hover:text-slate-200" onclick={() => { sortDir = sortDir === 'asc' ? 'desc' : 'asc'; load(); }}>
				{sortDir === 'asc' ? '↑' : '↓'}
			</button>
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
					<button class="btn btn-secondary text-xs" onclick={() => openCreate('container')}>Add container</button>
					<button class="btn btn-primary text-xs" onclick={() => openCreate('item')}>Add item</button>
				</div>
			</div>
		{:else}
			<div class="divide-y divide-slate-800">
				{#each children as item (item.id)}
					<button
						class="flex w-full items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-slate-800/50"
						onclick={() => {
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
								<span class="truncate font-medium text-slate-100">{item.name ?? 'Unnamed'}</span>
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
								{#if item.category}
									<span>{item.category}</span>
								{/if}
								{#if item.tags.length > 0}
									{#each item.tags.slice(0, 2) as tag}
										<span class="rounded-full bg-slate-700/60 px-1.5 py-0.5 text-[10px]">{tag}</span>
									{/each}
									{#if item.tags.length > 2}
										<span class="text-[10px] text-slate-500">+{item.tags.length - 2}</span>
									{/if}
								{/if}
								{#if !item.system_barcode && !item.category && item.tags.length === 0}
									<span class="font-mono text-slate-600">#{item.id.slice(-4)}</span>
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
			{#if hasMore}
				<div class="px-4 py-3">
					<button class="btn btn-secondary w-full text-sm" onclick={loadMore} disabled={loadingMore}>
						{#if loadingMore}
							<span class="h-4 w-4 animate-spin rounded-full border-2 border-slate-600 border-t-indigo-500 inline-block"></span>
						{:else}
							Load more
						{/if}
					</button>
				</div>
			{/if}
			<p class="px-4 py-2 text-xs text-slate-500">{children.length}{hasMore ? '+' : ''} item{children.length !== 1 || hasMore ? 's' : ''}</p>
		{/if}
	</div>
</div>

<!-- Create form — full-screen overlay -->
{#if showCreate}
<div class="fixed inset-0 z-50 flex flex-col bg-slate-950">
	<header class="flex items-center gap-2 border-b border-slate-800 px-3 py-2">
		<button class="btn btn-icon text-slate-400" onclick={closeCreate} aria-label="Cancel">
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M18 6L6 18M6 6l12 12" />
			</svg>
		</button>
		<h2 class="flex-1 text-base font-semibold text-slate-100">
			New {createType === 'container' ? 'container' : 'item'}
		</h2>
		<button class="btn btn-primary text-xs" onclick={submitCreate} disabled={creating}>
			{creating ? 'Creating…' : 'Create'}
		</button>
	</header>

	<div class="flex-1 overflow-y-auto p-4">
		{#if createError}
			<div class="mb-3 rounded-lg bg-red-950 px-3 py-2 text-sm text-red-300 border border-red-800">{createError}</div>
		{/if}

		<div class="space-y-4">
			<!-- Images -->
			<div>
				<p class="mb-2 text-sm font-medium text-slate-300">Photos</p>
				{#if createImagePreviews.length > 0}
					<div class="mb-3 flex gap-2 overflow-x-auto pb-1">
						{#each createImagePreviews as preview, idx}
							<div class="relative flex-shrink-0">
								<img src={preview} alt="Preview {idx + 1}" class="h-20 w-20 rounded-lg object-cover" />
								<button
									class="absolute -top-1.5 -right-1.5 flex h-5 w-5 items-center justify-center rounded-full bg-red-600 text-white text-xs"
									onclick={() => removeCreateImage(idx)}
								>
									&times;
								</button>
							</div>
						{/each}
					</div>
				{/if}
				<label class="btn btn-secondary w-full cursor-pointer text-sm">
					<svg class="mr-1.5 h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
						<path d="M23 19a2 2 0 0 1-2 2H3a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4l2-3h6l2 3h4a2 2 0 0 1 2 2z" />
						<circle cx="12" cy="13" r="4" />
					</svg>
					Add photos
					<input type="file" accept="image/*" multiple class="hidden" onchange={handleImageAdd} />
				</label>
			</div>

			<!-- Name -->
			<div>
				<label class="mb-1 block text-sm font-medium text-slate-300" for="create-name">Name *</label>
				<input
					id="create-name"
					class="input"
					bind:value={createName}
					placeholder={createType === 'container' ? 'e.g. Garage shelf' : 'e.g. Cordless drill'}
				/>
			</div>

			<!-- Description -->
			<div>
				<label class="mb-1 block text-sm font-medium text-slate-300" for="create-desc">Description</label>
				<textarea id="create-desc" class="input min-h-16 resize-y" bind:value={createDescription} placeholder="Optional" rows="2"></textarea>
			</div>

			<!-- Category -->
			{#if categories.length > 0}
				<div>
					<label class="mb-1 block text-sm font-medium text-slate-300" for="create-category">Category</label>
					<select id="create-category" class="input" bind:value={createCategoryId}>
						<option value={null}>None</option>
						{#each categories as cat (cat.id)}
							<option value={cat.id}>{cat.name}</option>
						{/each}
					</select>
				</div>
			{/if}

			<!-- Condition (items only) -->
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


					<!-- Fungible (items only) -->
					{#if createType === 'item'}
						<label class="flex items-center justify-between card p-3 cursor-pointer">
							<div>
								<p class="text-sm font-medium text-slate-300">Fungible</p>
								<p class="text-xs text-slate-500">Track quantity (e.g. consumables)</p>
							</div>
							<input type="checkbox" class="h-5 w-5 rounded border-slate-600 bg-slate-800 text-indigo-500" bind:checked={createIsFungible} />
						</label>
						{#if createIsFungible}
							<div class="grid grid-cols-2 gap-3">
								<div>
									<label class="mb-1 block text-xs text-slate-400" for="c-qty">Initial quantity</label>
									<input id="c-qty" class="input text-sm" type="number" min="0" bind:value={createFungibleQty} placeholder="0" />
								</div>
								<div>
									<label class="mb-1 block text-xs text-slate-400" for="c-unit">Unit</label>
									<input id="c-unit" class="input text-sm" bind:value={createFungibleUnit} placeholder="e.g. pieces" />
								</div>
							</div>
						{/if}
					{/if}

					<!-- Valuation -->
					<button type="button" class="text-xs text-slate-500 hover:text-slate-300" onclick={() => { showCreateAdvanced = !showCreateAdvanced; }}>
						{showCreateAdvanced ? 'Hide' : 'Show'} valuation fields
					</button>
					{#if showCreateAdvanced}
						<div class="grid grid-cols-2 gap-3">
							<div>
								<label class="mb-1 block text-xs text-slate-400" for="c-cost">Cost</label>
								<input id="c-cost" class="input text-sm" type="number" step="0.01" min="0" bind:value={createAcqCost} placeholder="0.00" />
							</div>
							<div>
								<label class="mb-1 block text-xs text-slate-400" for="c-currency">Currency</label>
								<input id="c-currency" class="input text-sm" bind:value={createCurrency} placeholder="USD" />
							</div>
						</div>
					{/if}

<!-- Tags -->
			{#if allTags.length > 0}
				<div>
					<p class="mb-2 text-sm font-medium text-slate-300">Tags</p>
					<div class="flex flex-wrap gap-1.5">
						{#each allTags as tag (tag.id)}
							<button
								type="button"
								class="rounded-full px-3 py-1 text-xs font-medium transition-colors
									{createSelectedTagIds.has(tag.id) ? 'bg-indigo-600 text-white' : 'bg-slate-700 text-slate-300 hover:bg-slate-600'}"
								onclick={() => toggleCreateTag(tag.id)}
							>
								{tag.name}
							</button>
						{/each}
					</div>
				</div>
			{/if}

			<!-- Container type (containers only) -->
			{#if createType === 'container' && containerTypes.length > 0}
				<div>
					<label class="mb-1 block text-sm font-medium text-slate-300" for="create-ctype">Container type</label>
					<select id="create-ctype" class="input" bind:value={createContainerTypeId} onchange={applyContainerType}>
						<option value={null}>None</option>
						{#each containerTypes as ct (ct.id)}
							<option value={ct.id}>{ct.icon ?? ''} {ct.name}</option>
						{/each}
					</select>
				</div>
			{/if}

			<!-- Location schema (containers only) -->
			{#if createType === 'container'}
				<LocationSchemaEditor bind:value={createLocationSchema} />
			{/if}

			<!-- Coordinate (when parent has a schema) -->
			{#if containerItem?.location_schema}
				<CoordinateInput schema={containerItem.location_schema} bind:value={createCoordinate} />
			{/if}

			<!-- Create button (bottom) -->
			<button class="btn btn-primary w-full" onclick={submitCreate} disabled={creating}>
				{creating ? 'Creating…' : `Create ${createType}`}
			</button>
		</div>
	</div>
</div>
{/if}
