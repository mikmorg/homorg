<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { api } from '$api/client.js';
	import { isAdmin } from '$stores/auth.js';
	import { toast } from '$stores/toast.js';
	import type { ContainerType } from '$api/types.js';
	import { schemaTypeLabel } from '$lib/coordinate-helpers.js';
	import LocationSchemaEditor from '$lib/components/LocationSchemaEditor.svelte';

	let types: ContainerType[] = [];
	let loading = true;
	let error = '';

	// Form
	let showForm = false;
	let editingId: string | null = null;
	let formName = '';
	let formDescription = '';
	let formIcon = '';
	let formPurpose = '';
	let formCapacity = '';
	let formWeight = '';
	let formLocationSchema: unknown | null = null;
	let formLoading = false;
	let formError = '';

	onMount(async () => {
		if (!$isAdmin) { goto('/'); return; }
		await loadTypes();
	});

	async function loadTypes() {
		loading = true;
		try {
			types = await api.containerTypes.list();
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to load';
		} finally {
			loading = false;
		}
	}

	function openCreate() {
		editingId = null;
		formName = '';
		formDescription = '';
		formIcon = '';
		formPurpose = '';
		formCapacity = '';
		formWeight = '';
		formLocationSchema = null;
		formError = '';
		showForm = true;
	}

	function openEdit(ct: ContainerType) {
		editingId = ct.id;
		formName = ct.name;
		formDescription = ct.description ?? '';
		formIcon = ct.icon ?? '';
		formPurpose = ct.purpose ?? '';
		formCapacity = ct.default_max_capacity_cc ?? '';
		formWeight = ct.default_max_weight_grams ?? '';
		formLocationSchema = ct.default_location_schema;
		formError = '';
		showForm = true;
	}

	async function saveType() {
		if (!formName.trim()) { formError = 'Name is required.'; return; }
		formLoading = true;
		formError = '';

		const body: Partial<ContainerType> = {
			name: formName.trim(),
			description: formDescription.trim() || null,
			icon: formIcon.trim() || null,
			purpose: formPurpose.trim() || null,
			default_max_capacity_cc: formCapacity || null,
			default_max_weight_grams: formWeight || null,
			default_location_schema: formLocationSchema
		};

		try {
			if (editingId) {
				await api.containerTypes.update(editingId, body);
			} else {
				await api.containerTypes.create(body);
			}
			showForm = false;
			await loadTypes();
		} catch (err) {
			formError = err instanceof Error ? err.message : 'Save failed';
		} finally {
			formLoading = false;
		}
	}

	async function deleteType(ct: ContainerType) {
		if (!confirm(`Delete container type "${ct.name}"?`)) return;
		try {
			await api.containerTypes.delete(ct.id);
			await loadTypes();
		} catch (err) {
			toast(err instanceof Error ? err.message : 'Delete failed', 'error');
		}
	}
</script>

<svelte:head>
	<title>Container Types — Homorg</title>
</svelte:head>

<div class="flex h-full flex-col">
	<header class="flex items-center gap-2 border-b border-slate-800 px-3 py-2">
		<a href="/admin" class="btn btn-icon text-slate-400" aria-label="Back">
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M15 18l-6-6 6-6" />
			</svg>
		</a>
		<h1 class="flex-1 text-base font-semibold text-slate-100">Container Types</h1>
		<button class="btn btn-primary text-xs" on:click={openCreate}>Add</button>
	</header>

	<div class="flex-1 overflow-y-auto">
		{#if loading}
			<div class="flex h-32 items-center justify-center">
				<div class="h-6 w-6 animate-spin rounded-full border-2 border-slate-600 border-t-indigo-500"></div>
			</div>
		{:else if error}
			<div class="m-4 rounded-lg bg-red-950 px-4 py-3 text-sm text-red-300 border border-red-800">{error}</div>
		{:else if types.length === 0}
			<div class="flex h-32 flex-col items-center justify-center gap-2 text-slate-500">
				<p class="text-sm">No container types yet</p>
				<button class="btn btn-secondary text-xs" on:click={openCreate}>Create first type</button>
			</div>
		{:else}
			<div class="divide-y divide-slate-800">
				{#each types as ct (ct.id)}
					<div class="px-4 py-3">
						<div class="flex items-center gap-3">
							{#if ct.icon}
								<span class="text-xl">{ct.icon}</span>
							{:else}
								<div class="flex h-8 w-8 items-center justify-center rounded-md bg-indigo-500/20 text-indigo-400 text-xs">
									<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
										<rect x="2" y="7" width="20" height="14" rx="2" />
									</svg>
								</div>
							{/if}
							<div class="flex-1 min-w-0">
								<span class="font-medium text-slate-100">{ct.name}</span>
								{#if ct.description}
									<p class="text-xs text-slate-400 truncate">{ct.description}</p>
								{/if}
								<div class="mt-1 flex gap-3 text-xs text-slate-500">
									{#if ct.default_max_capacity_cc}
										<span>{ct.default_max_capacity_cc} cc</span>
									{/if}
									{#if ct.default_max_weight_grams}
										<span>{ct.default_max_weight_grams} g</span>
									{/if}
									{#if ct.purpose}
										<span>{ct.purpose}</span>
									{/if}
									{#if ct.default_location_schema}
										<span>{schemaTypeLabel(ct.default_location_schema)}</span>
									{/if}
								</div>
							</div>
							<button class="btn btn-ghost text-xs text-slate-400 px-2 py-1" on:click={() => openEdit(ct)}>
								Edit
							</button>
							<button class="btn btn-ghost text-xs text-red-400 px-2 py-1" on:click={() => deleteType(ct)}>
								Delete
							</button>
						</div>
					</div>
				{/each}
			</div>
		{/if}
	</div>
</div>

<!-- Create/Edit form modal -->
{#if showForm}
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="fixed inset-0 z-50 flex flex-col justify-end bg-black/60" on:click|self={() => (showForm = false)} on:keydown={(e) => e.key === 'Escape' && (showForm = false)}>
	<div class="rounded-t-2xl bg-slate-900 p-4 pb-8">
		<div class="mb-4 flex items-center justify-between">
			<h2 class="text-base font-semibold text-slate-100">{editingId ? 'Edit' : 'New'} container type</h2>
			<button class="btn btn-icon text-slate-400" on:click={() => (showForm = false)} aria-label="Close">
				<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M18 6L6 18M6 6l12 12" />
				</svg>
			</button>
		</div>

		{#if formError}
			<div class="mb-3 rounded-lg bg-red-950 px-3 py-2 text-sm text-red-300 border border-red-800">{formError}</div>
		{/if}

		<div class="space-y-3">
			<div>
				<label class="mb-1 block text-sm font-medium text-slate-300" for="ct-name">Name *</label>
				<input id="ct-name" class="input" bind:value={formName} placeholder="e.g. Shelf, Drawer, Box" disabled={formLoading} />
			</div>
			<div>
				<label class="mb-1 block text-sm font-medium text-slate-300" for="ct-desc">Description</label>
				<input id="ct-desc" class="input" bind:value={formDescription} placeholder="Optional" disabled={formLoading} />
			</div>
			<div class="flex gap-3">
				<div class="flex-1">
					<label class="mb-1 block text-sm font-medium text-slate-300" for="ct-icon">Icon</label>
					<input id="ct-icon" class="input" bind:value={formIcon} placeholder="📦" disabled={formLoading} />
				</div>
				<div class="flex-1">
					<label class="mb-1 block text-sm font-medium text-slate-300" for="ct-purpose">Purpose</label>
					<input id="ct-purpose" class="input" bind:value={formPurpose} placeholder="storage" disabled={formLoading} />
				</div>
			</div>
			<div class="flex gap-3">
				<div class="flex-1">
					<label class="mb-1 block text-sm font-medium text-slate-300" for="ct-cap">Capacity (cc)</label>
					<input id="ct-cap" class="input" type="number" step="0.01" bind:value={formCapacity} disabled={formLoading} />
				</div>
				<div class="flex-1">
					<label class="mb-1 block text-sm font-medium text-slate-300" for="ct-weight">Max weight (g)</label>
					<input id="ct-weight" class="input" type="number" step="0.01" bind:value={formWeight} disabled={formLoading} />
				</div>
			</div>
			<LocationSchemaEditor bind:value={formLocationSchema} />
			<button class="btn btn-primary w-full" on:click={saveType} disabled={formLoading}>
				{formLoading ? 'Saving…' : editingId ? 'Update' : 'Create'}
			</button>
		</div>
	</div>
</div>
{/if}
