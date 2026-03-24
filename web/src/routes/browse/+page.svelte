<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import { api } from '$api/client.js';
	import type { Item, ItemSummary, CreateItemRequest, Condition, Category, Tag, ContainerType } from '$api/types.js';
	import { CONDITIONS } from '$api/types.js';
	import CoordinateInput from '$lib/components/CoordinateInput.svelte';
	import LocationSchemaEditor from '$lib/components/LocationSchemaEditor.svelte';
	import { toast } from '$stores/toast.js';

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
	let createCategoryId: string | null = null;
	let createSelectedTagIds: Set<string> = new Set();
	let createContainerTypeId: string | null = null;
	let createLocationSchema: unknown | null = null;
	let createImages: File[] = [];
	let createImagePreviews: string[] = [];
	let createIsFungible = false;
	let createFungibleUnit = '';
	let createFungibleQty = '';
	let createAcqCost = '';
	let createCurrency = '';
	let showCreateAdvanced = false;
	let creating = false;
	let createError = '';

	// Taxonomy data (loaded once for the create form)
	let categories: Category[] = [];
	let allTags: Tag[] = [];
	let containerTypes: ContainerType[] = [];
	let taxonomyLoaded = false;

	// Sort state
	let sortBy: 'name' | 'created_at' | 'category' = 'name';
	let sortDir: 'asc' | 'desc' = 'asc';

	$: containerId = $page.url.searchParams.get('id') ?? ROOT_ID;

	$: if (containerId) {
		load();
	}

	onMount(load);

	async function load() {
		loading = true;
		error = '';
		try {
			const res = await api.containers.children(containerId, { limit: 200, sort_by: sortBy, sort_dir: sortDir });
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
		createImages = [];
		createImagePreviews.forEach((url) => URL.revokeObjectURL(url));
		createImagePreviews = [];
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
				if (createFungibleQty) body.fungible_quantity = parseInt(createFungibleQty);
			}
			if (createAcqCost) body.acquisition_cost = parseFloat(createAcqCost);
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

			showCreate = false;
			createImagePreviews.forEach((url) => URL.revokeObjectURL(url));
			createImagePreviews = [];
			createIsFungible = false;
			createFungibleUnit = '';
			if (postCreateWarnings.length > 0) {
				for (const w of postCreateWarnings) toast(w, 'error');
			}
			createFungibleQty = '';
			createAcqCost = '';
			createCurrency = '';
			showCreateAdvanced = false;
			createImages = [];
			toast(createType === 'container' ? 'Container created' : 'Item created', 'success');
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

<svelte:window on:keydown={(e) => { if (e.key === "Escape") { if (showCreate) showCreate = false; } }} />

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

	<!-- Sort bar -->
	{#if children.length > 1}
		<div class="flex items-center gap-2 border-b border-slate-800 px-3 py-1.5">
			<span class="text-xs text-slate-500">Sort:</span>
			<select class="bg-transparent text-xs text-slate-300 border-0 py-0 pr-6 pl-0 focus:ring-0" bind:value={sortBy} on:change={load}>
				<option value="name">Name</option>
				<option value="created_at">Created</option>
				<option value="category">Category</option>
			</select>
			<button class="text-xs text-slate-400 hover:text-slate-200" on:click={() => { sortDir = sortDir === 'asc' ? 'desc' : 'asc'; load(); }}>
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
							</div>
						</div>

						<!-- Chevron -->
						<svg class="h-4 w-4 flex-shrink-0 text-slate-600" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M9 18l6-6-6-6" />
						</svg>
					</button>
				{/each}
			</div>
			<p class="px-4 py-2 text-xs text-slate-500">{children.length} item{children.length !== 1 ? 's' : ''}</p>
		{/if}
	</div>
</div>

<!-- Create form — full-screen overlay -->
{#if showCreate}
<div class="fixed inset-0 z-50 flex flex-col bg-slate-950">
	<header class="flex items-center gap-2 border-b border-slate-800 px-3 py-2">
		<button class="btn btn-icon text-slate-400" on:click={() => { showCreate = false; createImagePreviews.forEach((u) => URL.revokeObjectURL(u)); }} aria-label="Cancel">
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M18 6L6 18M6 6l12 12" />
			</svg>
		</button>
		<h2 class="flex-1 text-base font-semibold text-slate-100">
			New {createType === 'container' ? 'container' : 'item'}
		</h2>
		<button class="btn btn-primary text-xs" on:click={submitCreate} disabled={creating}>
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
									on:click={() => removeCreateImage(idx)}
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
					<input type="file" accept="image/*" multiple class="hidden" on:change={handleImageAdd} />
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
					<button type="button" class="text-xs text-slate-500 hover:text-slate-300" on:click={() => { showCreateAdvanced = !showCreateAdvanced; }}>
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
								on:click={() => toggleCreateTag(tag.id)}
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
					<select id="create-ctype" class="input" bind:value={createContainerTypeId} on:change={applyContainerType}>
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
			<button class="btn btn-primary w-full" on:click={submitCreate} disabled={creating}>
				{creating ? 'Creating…' : `Create ${createType}`}
			</button>
		</div>
	</div>
</div>
{/if}
