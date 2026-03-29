<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { api } from '$api/client.js';
	import { isAdmin } from '$stores/auth.js';
	import { toast } from '$stores/toast.js';
	import type { Category } from '$api/types.js';

	let categories: Category[] = $state([]);
	let loading = $state(true);
	let error = $state('');

	// Create/edit form
	let showForm = $state(false);
	let editingId: string | null = $state(null);
	let formName = $state('');
	let formDescription = $state('');
	let formParentId: string | null = $state(null);
	let formLoading = $state(false);
	let formError = $state('');

	onMount(async () => {
		if (!$isAdmin) { goto('/'); return; }
		await loadCategories();
	});

	async function loadCategories() {
		loading = true;
		try {
			categories = await api.categories.list();
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
		formParentId = null;
		formError = '';
		showForm = true;
	}

	function openEdit(cat: Category) {
		editingId = cat.id;
		formName = cat.name;
		formDescription = cat.description ?? '';
		formParentId = cat.parent_category_id ?? null;
		formError = '';
		showForm = true;
	}

	async function saveCategory() {
		if (!formName.trim()) { formError = 'Name is required.'; return; }
		formLoading = true;
		formError = '';
		try {
			if (editingId) {
				await api.categories.update(editingId, {
					name: formName.trim(),
					description: formDescription.trim() || null,
					parent_category_id: formParentId
				});
			} else {
				await api.categories.create(
					formName.trim(),
					formDescription.trim() || undefined,
					formParentId ?? undefined
				);
			}
			showForm = false;
			await loadCategories();
			toast(editingId ? 'Category updated' : 'Category created', 'success');
		} catch (err) {
			formError = err instanceof Error ? err.message : 'Save failed';
		} finally {
			formLoading = false;
		}
	}

	async function deleteCategory(cat: Category) {
		if (!confirm(`Delete category "${cat.name}"?`)) return;
		try {
			await api.categories.delete(cat.id);
			await loadCategories();
			toast('Category deleted', 'success');
		} catch (err) {
			toast(err instanceof Error ? err.message : 'Delete failed', 'error');
		}
	}
</script>

<svelte:head>
	<title>Categories — Homorg</title>
</svelte:head>

<div class="flex h-full flex-col">
	<header class="flex items-center gap-2 border-b border-slate-800 px-3 py-2">
		<a href="/admin" class="btn btn-icon text-slate-400" aria-label="Back">
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M15 18l-6-6 6-6" />
			</svg>
		</a>
		<h1 class="flex-1 text-base font-semibold text-slate-100">Categories</h1>
		<button class="btn btn-primary text-xs" onclick={openCreate}>Add</button>
	</header>

	<div class="flex-1 overflow-y-auto">
		{#if loading}
			<div class="flex h-32 items-center justify-center">
				<div class="h-6 w-6 animate-spin rounded-full border-2 border-slate-600 border-t-indigo-500"></div>
			</div>
		{:else if error}
			<div class="m-4 rounded-lg bg-red-950 px-4 py-3 text-sm text-red-300 border border-red-800">{error}</div>
		{:else if categories.length === 0}
			<div class="flex h-32 flex-col items-center justify-center gap-2 text-slate-500">
				<p class="text-sm">No categories yet</p>
				<button class="btn btn-secondary text-xs" onclick={openCreate}>Create first category</button>
			</div>
		{:else}
			<div class="divide-y divide-slate-800">
				{#each categories as cat (cat.id)}
					<div class="flex items-center gap-3 px-4 py-3">
						<div class="flex-1 min-w-0">
							<div class="flex items-center gap-2">
								<span class="font-medium text-slate-100 truncate">{cat.name}</span>
								{#if cat.item_count !== undefined}
									<span class="text-xs text-slate-500">{cat.item_count} items</span>
								{/if}
							</div>
							{#if cat.description}
								<p class="text-xs text-slate-400 truncate">{cat.description}</p>
							{/if}
							{#if cat.parent_category_id}
								{@const parent = categories.find(c => c.id === cat.parent_category_id)}
								{#if parent}
									<p class="text-xs text-slate-500">Parent: {parent.name}</p>
								{/if}
							{/if}
						</div>
						<button class="btn btn-ghost text-xs text-slate-400 px-2 py-1" onclick={() => openEdit(cat)}>
							Edit
						</button>
						<button class="btn btn-ghost text-xs text-red-400 px-2 py-1" onclick={() => deleteCategory(cat)}>
							Delete
						</button>
					</div>
				{/each}
			</div>
		{/if}
	</div>
</div>

<!-- Create/Edit form modal -->
{#if showForm}
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="fixed inset-0 z-50 flex flex-col justify-end bg-black/60"
	onclick={(e) => { if (e.target === e.currentTarget) showForm = false; }}
	onkeydown={(e) => e.key === 'Escape' && (showForm = false)}
>
	<div class="rounded-t-2xl bg-slate-900 p-4 pb-8" role="dialog" aria-modal="true" aria-labelledby="cat-form-title">
		<div class="mb-4 flex items-center justify-between">
			<h2 id="cat-form-title" class="text-base font-semibold text-slate-100">{editingId ? 'Edit' : 'New'} category</h2>
			<button class="btn btn-icon text-slate-400" onclick={() => (showForm = false)} aria-label="Close">
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
				<label class="mb-1 block text-sm font-medium text-slate-300" for="cat-name">Name *</label>
				<input id="cat-name" class="input" bind:value={formName} placeholder="Category name" disabled={formLoading} />
			</div>
			<div>
				<label class="mb-1 block text-sm font-medium text-slate-300" for="cat-desc">Description</label>
				<input id="cat-desc" class="input" bind:value={formDescription} placeholder="Optional" disabled={formLoading} />
			</div>
			<div>
				<label class="mb-1 block text-sm font-medium text-slate-300" for="cat-parent">Parent category</label>
				<select id="cat-parent" class="input" bind:value={formParentId} disabled={formLoading}>
					<option value={null}>None (top-level)</option>
					{#each categories.filter(c => c.id !== editingId) as cat (cat.id)}
						<option value={cat.id}>{cat.name}</option>
					{/each}
				</select>
			</div>
			<button class="btn btn-primary w-full" onclick={saveCategory} disabled={formLoading}>
				{formLoading ? 'Saving…' : editingId ? 'Update' : 'Create'}
			</button>
		</div>
	</div>
</div>
{/if}
