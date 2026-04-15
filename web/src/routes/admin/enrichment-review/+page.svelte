<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { api } from '$api/client.js';
	import { isAdmin } from '$stores/auth.js';
	import { toast } from '$stores/toast.js';
	import type { Item, AiSuggestions } from '$api/types.js';

	let items: Item[] = $state([]);
	let total = $state(0);
	let loading = $state(true);
	let error = $state('');
	let busyId: string | null = $state(null);

	// Per-item per-field accept flags. Map: itemId -> { name, description, ... }
	let accept: Record<string, Record<string, boolean>> = $state({});

	const PAGE_SIZE = 50;
	let offset = $state(0);

	onMount(async () => {
		if (!$isAdmin) {
			goto('/');
			return;
		}
		await load();
	});

	async function load() {
		loading = true;
		error = '';
		try {
			const resp = await api.enrichment.listReview({ limit: PAGE_SIZE, offset });
			items = resp.items;
			total = resp.total;
			// Default: accept everything for items that have a suggestion.
			accept = Object.fromEntries(
				items
					.filter((i) => i.ai_suggestions)
					.map((i) => [i.id, defaultAccept(i.ai_suggestions!)])
			);
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to load review queue';
		} finally {
			loading = false;
		}
	}

	function defaultAccept(s: AiSuggestions): Record<string, boolean> {
		return {
			name: s.name !== undefined,
			description: s.description !== undefined,
			category: s.category !== undefined,
			tags: Array.isArray(s.tags) && s.tags.length > 0,
			metadata:
				s.metadata_additions !== undefined &&
				s.metadata_additions !== null &&
				Object.keys(s.metadata_additions as Record<string, unknown>).length > 0
		};
	}

	function confidenceClass(c: number): string {
		if (c < 0.5) return 'bg-red-900 text-red-300';
		if (c < 0.8) return 'bg-yellow-900 text-yellow-300';
		return 'bg-emerald-900 text-emerald-300';
	}

	async function approve(item: Item) {
		if (!item.ai_suggestions) return;
		const a = accept[item.id] ?? defaultAccept(item.ai_suggestions);
		busyId = item.id;
		try {
			await api.enrichment.approve(item.id, a);
			toast('Suggestions applied', 'success');
			await load();
		} catch (err) {
			toast(err instanceof Error ? err.message : 'Approve failed', 'error');
		} finally {
			busyId = null;
		}
	}

	async function reject(item: Item) {
		busyId = item.id;
		try {
			await api.enrichment.reject(item.id);
			toast('Suggestions rejected', 'success');
			await load();
		} catch (err) {
			toast(err instanceof Error ? err.message : 'Reject failed', 'error');
		} finally {
			busyId = null;
		}
	}

	async function rerun(item: Item) {
		busyId = item.id;
		try {
			await api.enrichment.rerun(item.id);
			toast('Re-run queued', 'success');
		} catch (err) {
			toast(err instanceof Error ? err.message : 'Re-run failed', 'error');
		} finally {
			busyId = null;
		}
	}

	async function addDiscoveredCode(item: Item, codeType: string, value: string) {
		busyId = item.id;
		try {
			await api.items.addExternalCode(item.id, codeType, value);
			toast(`Added ${codeType}: ${value}`, 'success');
		} catch (err) {
			toast(err instanceof Error ? err.message : 'Add code failed', 'error');
		} finally {
			busyId = null;
		}
	}

	function toggleField(itemId: string, field: string) {
		const map = accept[itemId] ?? {};
		accept = { ...accept, [itemId]: { ...map, [field]: !map[field] } };
	}

	async function nextPage() {
		offset += PAGE_SIZE;
		await load();
	}
	async function prevPage() {
		offset = Math.max(0, offset - PAGE_SIZE);
		await load();
	}
</script>

<svelte:head>
	<title>Review queue — Homorg</title>
</svelte:head>

<div class="flex h-full flex-col">
	<header class="flex items-center justify-between border-b border-slate-800 px-4 py-3">
		<div>
			<h1 class="text-lg font-semibold text-slate-100">Enrichment review queue</h1>
			<p class="text-xs text-slate-400">{total} item{total === 1 ? '' : 's'} awaiting review</p>
		</div>
		<a href="/admin" class="text-sm text-indigo-400 hover:text-indigo-300">← Admin</a>
	</header>

	<div class="flex-1 overflow-y-auto p-4 space-y-4">
		{#if loading}
			<div class="flex h-16 items-center justify-center">
				<div
					class="h-5 w-5 animate-spin rounded-full border-2 border-slate-600 border-t-indigo-500"
				></div>
			</div>
		{:else if error}
			<div class="rounded-lg bg-red-950 px-4 py-3 text-sm text-red-300 border border-red-800">
				{error}
			</div>
		{:else if items.length === 0}
			<p class="text-center text-sm text-slate-400 py-8">No items awaiting review.</p>
		{:else}
			{#each items as item (item.id)}
				{@const s = item.ai_suggestions!}
				{@const fields = accept[item.id] ?? defaultAccept(s)}
				<div class="card p-4 space-y-3">
					<div class="flex items-start justify-between gap-3">
						<div class="flex-1 min-w-0">
							<a
								href="/browse/item/{item.id}"
								class="text-sm font-medium text-slate-100 hover:text-indigo-400"
							>
								{item.name ?? '(unnamed)'}
							</a>
							{#if item.category}
								<p class="text-xs text-slate-400">{item.category}</p>
							{/if}
						</div>
						<span class="text-xs px-2 py-0.5 rounded-full {confidenceClass(s.confidence)}">
							{(s.confidence * 100).toFixed(0)}%
						</span>
					</div>

					<!-- Field-by-field diff -->
					<div class="space-y-2 text-sm">
						{#if s.name !== undefined}
							<label class="flex items-start gap-2">
								<input
									type="checkbox"
									checked={fields.name}
									onchange={() => toggleField(item.id, 'name')}
									class="mt-1"
								/>
								<div class="flex-1 min-w-0">
									<p class="text-xs text-slate-400 uppercase tracking-wide">Name</p>
									<p class="text-slate-500 line-through">{item.name ?? '—'}</p>
									<p class="text-slate-100">{s.name}</p>
								</div>
							</label>
						{/if}
						{#if s.description !== undefined}
							<label class="flex items-start gap-2">
								<input
									type="checkbox"
									checked={fields.description}
									onchange={() => toggleField(item.id, 'description')}
									class="mt-1"
								/>
								<div class="flex-1 min-w-0">
									<p class="text-xs text-slate-400 uppercase tracking-wide">Description</p>
									<p class="text-slate-500 line-through">{item.description ?? '—'}</p>
									<p class="text-slate-100">{s.description}</p>
								</div>
							</label>
						{/if}
						{#if s.category !== undefined}
							<label class="flex items-start gap-2">
								<input
									type="checkbox"
									checked={fields.category}
									onchange={() => toggleField(item.id, 'category')}
									class="mt-1"
								/>
								<div class="flex-1 min-w-0">
									<p class="text-xs text-slate-400 uppercase tracking-wide">Category</p>
									<p class="text-slate-500 line-through">{item.category ?? '—'}</p>
									<p class="text-slate-100">{s.category}</p>
								</div>
							</label>
						{/if}
						{#if s.tags && s.tags.length > 0}
							<label class="flex items-start gap-2">
								<input
									type="checkbox"
									checked={fields.tags}
									onchange={() => toggleField(item.id, 'tags')}
									class="mt-1"
								/>
								<div class="flex-1 min-w-0">
									<p class="text-xs text-slate-400 uppercase tracking-wide">Tags (union)</p>
									<p class="text-slate-100">{s.tags.join(', ')}</p>
								</div>
							</label>
						{/if}
						{#if s.metadata_additions && Object.keys(s.metadata_additions as Record<string, unknown>).length > 0}
							<label class="flex items-start gap-2">
								<input
									type="checkbox"
									checked={fields.metadata}
									onchange={() => toggleField(item.id, 'metadata')}
									class="mt-1"
								/>
								<div class="flex-1 min-w-0">
									<p class="text-xs text-slate-400 uppercase tracking-wide">Metadata additions</p>
									<pre class="text-xs text-slate-300 bg-slate-900 p-2 rounded overflow-x-auto">{JSON.stringify(
											s.metadata_additions,
											null,
											2
										)}</pre>
								</div>
							</label>
						{/if}
					</div>

					<!-- Discovered codes -->
					{#if s.discovered_codes && s.discovered_codes.length > 0}
						<div class="border-t border-slate-800 pt-3 space-y-1">
							<p class="text-xs text-slate-400 uppercase tracking-wide">Discovered codes</p>
							{#each s.discovered_codes as [codeType, value]}
								<div class="flex items-center justify-between text-sm">
									<span class="text-slate-300"
										><span class="text-slate-500">{codeType}</span>: {value}</span
									>
									<button
										class="text-xs text-indigo-400 hover:text-indigo-300 disabled:opacity-50"
										onclick={() => addDiscoveredCode(item, codeType, value)}
										disabled={busyId === item.id}
									>
										+ Add
									</button>
								</div>
							{/each}
						</div>
					{/if}

					<!-- Reasoning -->
					{#if s.reasoning}
						<details class="text-xs text-slate-400">
							<summary class="cursor-pointer hover:text-slate-300">Reasoning</summary>
							<p class="mt-1 whitespace-pre-wrap">{s.reasoning}</p>
						</details>
					{/if}

					<!-- Actions -->
					<div class="flex items-center gap-2 border-t border-slate-800 pt-3">
						<button
							class="btn-primary disabled:opacity-50"
							onclick={() => approve(item)}
							disabled={busyId === item.id}
						>
							Approve selected
						</button>
						<button
							class="btn-secondary disabled:opacity-50"
							onclick={() => reject(item)}
							disabled={busyId === item.id}
						>
							Reject
						</button>
						<button
							class="text-sm text-slate-400 hover:text-slate-200 ml-auto disabled:opacity-50"
							onclick={() => rerun(item)}
							disabled={busyId === item.id}
						>
							Re-run
						</button>
					</div>
				</div>
			{/each}

			{#if total > PAGE_SIZE}
				<div class="flex items-center justify-between pt-2">
					<button
						class="btn-secondary disabled:opacity-50"
						onclick={prevPage}
						disabled={offset === 0}
					>
						← Prev
					</button>
					<span class="text-xs text-slate-400"
						>{offset + 1}–{Math.min(offset + PAGE_SIZE, total)} of {total}</span
					>
					<button
						class="btn-secondary disabled:opacity-50"
						onclick={nextPage}
						disabled={offset + PAGE_SIZE >= total}
					>
						Next →
					</button>
				</div>
			{/if}
		{/if}
	</div>
</div>
