<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { api } from '$api/client.js';
	import { isAdmin } from '$stores/auth.js';
	import type { Tag } from '$api/types.js';

	let tags: Tag[] = [];
	let loading = true;
	let error = '';

	// Create
	let newTagName = '';
	let creating = false;

	// Rename
	let renamingId: string | null = null;
	let renameValue = '';
	let renaming = false;

	onMount(async () => {
		if (!$isAdmin) { goto('/'); return; }
		await loadTags();
	});

	async function loadTags() {
		loading = true;
		try {
			tags = await api.tags.list();
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to load';
		} finally {
			loading = false;
		}
	}

	async function createTag() {
		if (!newTagName.trim()) return;
		creating = true;
		try {
			await api.tags.create(newTagName.trim());
			newTagName = '';
			await loadTags();
		} catch (err) {
			alert(err instanceof Error ? err.message : 'Create failed');
		} finally {
			creating = false;
		}
	}

	function startRename(tag: Tag) {
		renamingId = tag.id;
		renameValue = tag.name;
	}

	async function saveRename() {
		if (!renamingId || !renameValue.trim()) return;
		renaming = true;
		try {
			await api.tags.rename(renamingId, renameValue.trim());
			renamingId = null;
			await loadTags();
		} catch (err) {
			alert(err instanceof Error ? err.message : 'Rename failed');
		} finally {
			renaming = false;
		}
	}

	async function deleteTag(tag: Tag) {
		if (!confirm(`Delete tag "${tag.name}"?`)) return;
		try {
			await api.tags.delete(tag.id);
			await loadTags();
		} catch (err) {
			alert(err instanceof Error ? err.message : 'Delete failed');
		}
	}
</script>

<svelte:head>
	<title>Tags — Homorg</title>
</svelte:head>

<div class="flex h-full flex-col">
	<header class="flex items-center gap-2 border-b border-slate-800 px-3 py-2">
		<a href="/admin" class="btn btn-icon text-slate-400">
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M15 18l-6-6 6-6" />
			</svg>
		</a>
		<h1 class="flex-1 text-base font-semibold text-slate-100">Tags</h1>
	</header>

	<!-- Inline create -->
	<form class="flex items-center gap-2 border-b border-slate-800 px-4 py-2" on:submit|preventDefault={createTag}>
		<input class="input flex-1" placeholder="New tag name" bind:value={newTagName} disabled={creating} />
		<button type="submit" class="btn btn-primary text-xs" disabled={creating || !newTagName.trim()}>
			{creating ? '…' : 'Add'}
		</button>
	</form>

	<div class="flex-1 overflow-y-auto">
		{#if loading}
			<div class="flex h-32 items-center justify-center">
				<div class="h-6 w-6 animate-spin rounded-full border-2 border-slate-600 border-t-indigo-500"></div>
			</div>
		{:else if error}
			<div class="m-4 rounded-lg bg-red-950 px-4 py-3 text-sm text-red-300 border border-red-800">{error}</div>
		{:else if tags.length === 0}
			<div class="flex h-32 items-center justify-center text-sm text-slate-500">No tags yet</div>
		{:else}
			<div class="divide-y divide-slate-800">
				{#each tags as tag (tag.id)}
					<div class="flex items-center gap-3 px-4 py-3">
						{#if renamingId === tag.id}
							<input
								class="input flex-1 text-sm"
								bind:value={renameValue}
								on:keydown={(e) => e.key === 'Enter' && saveRename()}
								disabled={renaming}
								autofocus
							/>
							<button class="btn btn-primary text-xs px-2 py-1" on:click={saveRename} disabled={renaming}>
								Save
							</button>
							<button class="btn btn-ghost text-xs px-2 py-1" on:click={() => (renamingId = null)}>
								Cancel
							</button>
						{:else}
							<div class="flex-1 min-w-0">
								<div class="flex items-center gap-2">
									<span class="font-medium text-slate-100">{tag.name}</span>
									{#if tag.item_count !== undefined}
										<span class="text-xs text-slate-500">{tag.item_count} items</span>
									{/if}
								</div>
							</div>
							<button class="btn btn-ghost text-xs text-slate-400 px-2 py-1" on:click={() => startRename(tag)}>
								Rename
							</button>
							<button class="btn btn-ghost text-xs text-red-400 px-2 py-1" on:click={() => deleteTag(tag)}>
								Delete
							</button>
						{/if}
					</div>
				{/each}
			</div>
		{/if}
	</div>
</div>
