<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { api } from '$api/client.js';
	import type { Item, AncestorEntry } from '$api/types.js';

	const itemId = $page.params.itemId!;
	let item: Item | null = null;
	let ancestors: AncestorEntry[] = [];
	let loading = true;
	let error = '';
	let deleting = false;

	onMount(async () => {
		try {
			const [fetchedItem, ancs] = await Promise.all([
				api.items.get(itemId),
				api.containers.ancestors(itemId)
			]);
			item = fetchedItem;
			ancestors = ancs;
		} catch (err) {
			error = err instanceof Error ? err.message : 'Item not found';
		} finally {
			loading = false;
		}
	});

	async function deleteItem() {
		if (!item) return;
		if (!confirm(`Delete "${item.name}"? This action can be undone.`)) return;
		deleting = true;
		try {
			await api.items.delete(itemId);
			history.back();
		} catch (err) {
			alert(err instanceof Error ? err.message : 'Delete failed');
			deleting = false;
		}
	}

	function formatDate(iso: string) {
		return new Date(iso).toLocaleString(undefined, {
			year: 'numeric', month: 'short', day: 'numeric',
			hour: '2-digit', minute: '2-digit'
		});
	}

	const CONDITION_LABELS: Record<string, string> = {
		new: 'New', like_new: 'Like new', good: 'Good',
		fair: 'Fair', poor: 'Poor', broken: 'Broken'
	};
</script>

<svelte:head>
	<title>{item?.name ?? 'Item'} — Homorg</title>
</svelte:head>

<div class="flex h-full flex-col">
	<header class="flex items-center gap-2 border-b border-slate-800 px-3 py-2">
		<button class="btn btn-icon text-slate-400" on:click={() => history.back()} aria-label="Back">
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M15 18l-6-6 6-6" />
			</svg>
		</button>
		<h1 class="flex-1 text-base font-semibold text-slate-100 truncate">
			{item?.name ?? 'Loading…'}
		</h1>
		{#if item}
			<a href="/browse/item/{itemId}/edit" class="btn btn-secondary text-xs">Edit</a>
		{/if}
	</header>

	<div class="flex-1 overflow-y-auto p-4">
		{#if loading}
			<div class="flex h-32 items-center justify-center">
				<div class="h-6 w-6 animate-spin rounded-full border-2 border-slate-600 border-t-indigo-500"></div>
			</div>
		{:else if error}
			<div class="rounded-lg bg-red-950 px-4 py-3 text-sm text-red-300 border border-red-800">{error}</div>
		{:else if item}
			<div class="space-y-4">
				<!-- Name + condition -->
				<div>
					<h2 class="text-xl font-semibold text-slate-100">{item.name}</h2>
					{#if item.condition}
						<span class="badge badge-{item.condition} mt-1">{CONDITION_LABELS[item.condition] ?? item.condition}</span>
					{/if}
				</div>

				<!-- Image -->
				{#if item.images && item.images.length > 0}
					<img src="/files/{item.images[0].path}" alt={item.images[0].caption ?? item.name} class="w-full rounded-lg object-cover max-h-48" />
				{/if}

				<!-- Location breadcrumb -->
				{#if ancestors.length > 0}
					<div class="card p-3">
						<p class="mb-1 text-xs text-slate-400 uppercase tracking-wide">Location</p>
						<div class="flex flex-wrap items-center gap-1 text-sm">
							{#each ancestors as a, i}
								<a href="/browse?id={a.id}" class="text-indigo-400 hover:underline">{a.name}</a>
								{#if i < ancestors.length - 1}
									<span class="text-slate-600">/</span>
								{/if}
							{/each}
						</div>
					</div>
				{/if}

				<!-- Properties grid -->
				<div class="card divide-y divide-slate-700">
					{#if item.fungible_quantity !== null}
						<div class="flex items-center justify-between px-3 py-2.5">
							<span class="text-sm text-slate-400">Quantity</span>
							<span class="text-sm font-medium text-slate-100">{item.fungible_quantity}{#if item.fungible_unit} {item.fungible_unit}{/if}</span>
						</div>
					{/if}
					{#if item.system_barcode}
						<div class="flex items-center justify-between px-3 py-2.5">
							<span class="text-sm text-slate-400">System barcode</span>
							<span class="text-xs font-mono text-slate-100">{item.system_barcode}</span>
						</div>
					{/if}
					<div class="flex items-center justify-between px-3 py-2.5">
						<span class="text-sm text-slate-400">Created</span>
						<span class="text-xs text-slate-300">{formatDate(item.created_at)}</span>
					</div>
					<div class="flex items-center justify-between px-3 py-2.5">
						<span class="text-sm text-slate-400">Updated</span>
						<span class="text-xs text-slate-300">{formatDate(item.updated_at)}</span>
					</div>
				</div>

				{#if item.description}
					<div>
						<p class="mb-1 text-xs text-slate-400 uppercase tracking-wide">Description</p>
						<p class="text-sm text-slate-300 whitespace-pre-wrap">{item.description}</p>
					</div>
				{/if}

				<!-- Tags -->
				{#if item.tags && item.tags.length > 0}
					<div>
						<p class="mb-2 text-xs text-slate-400 uppercase tracking-wide">Tags</p>
						<div class="flex flex-wrap gap-1">
							{#each item.tags as tag}
								<span class="badge">{tag}</span>
							{/each}
						</div>
					</div>
				{/if}

				<!-- External barcodes -->
				{#if item.external_codes && item.external_codes.length > 0}
					<div>
						<p class="mb-2 text-xs text-slate-400 uppercase tracking-wide">External barcodes</p>
						<div class="space-y-1">
							{#each item.external_codes as code}
								<span class="block text-xs font-mono text-slate-300">{code}</span>
							{/each}
						</div>
					</div>
				{/if}

				<!-- Danger zone -->
				<div class="pt-2">
					<button class="btn btn-danger w-full" on:click={deleteItem} disabled={deleting}>
						{deleting ? 'Deleting…' : 'Delete item'}
					</button>
				</div>
			</div>
		{/if}
	</div>
</div>
