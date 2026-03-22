<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { api } from '$api/client.js';
	import { authStore, isAdmin } from '$stores/auth.js';
	import type { StatsResponse } from '$api/types.js';

	let stats: StatsResponse | null = null;
	let loading = true;

	onMount(async () => {
		if (!$isAdmin) { goto('/'); return; }
		try {
			stats = await api.system.stats();
		} catch {
			stats = null;
		} finally {
			loading = false;
		}
	});

	async function logout() {
		const auth = $authStore;
		try { if (auth?.refresh_token) await api.auth.logout(auth.refresh_token); } catch { /* ignore */ }
		authStore.clear();
		goto('/login');
	}
</script>

<svelte:head>
	<title>Admin — Homorg</title>
</svelte:head>

<div class="flex h-full flex-col">
	<header class="border-b border-slate-800 px-4 py-3">
		<h1 class="text-lg font-semibold text-slate-100">Admin</h1>
	</header>

	<div class="flex-1 overflow-y-auto p-4 space-y-4">
		{#if loading}
			<div class="flex h-16 items-center justify-center">
				<div class="h-5 w-5 animate-spin rounded-full border-2 border-slate-600 border-t-indigo-500"></div>
			</div>
		{:else if stats}
			<!-- Stats cards -->
			<div class="grid grid-cols-2 gap-3">
				<div class="card p-4">
					<p class="text-xs text-slate-400">Total items</p>
					<p class="mt-1 text-2xl font-bold text-slate-100">{stats.total_items.toLocaleString()}</p>
				</div>
				<div class="card p-4">
					<p class="text-xs text-slate-400">Containers</p>
					<p class="mt-1 text-2xl font-bold text-slate-100">{stats.total_containers.toLocaleString()}</p>
				</div>
				<div class="card p-4">
					<p class="text-xs text-slate-400">Events</p>
					<p class="mt-1 text-2xl font-bold text-slate-100">{stats.total_events.toLocaleString()}</p>
				</div>
				<div class="card p-4">
					<p class="text-xs text-slate-400">Users</p>
					<p class="mt-1 text-2xl font-bold text-slate-100">{stats.total_users.toLocaleString()}</p>
				</div>
			</div>
		{/if}

		<!-- Navigation sections -->
		<div class="card divide-y divide-slate-700">
			<a href="/admin/users" class="flex items-center justify-between px-4 py-3 hover:bg-slate-700 transition-colors">
				<span class="text-sm font-medium text-slate-100">Users</span>
				<svg class="h-4 w-4 text-slate-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M9 18l6-6-6-6"/></svg>
			</a>
			<a href="/admin/categories" class="flex items-center justify-between px-4 py-3 hover:bg-slate-700 transition-colors">
				<span class="text-sm font-medium text-slate-100">Categories</span>
				<svg class="h-4 w-4 text-slate-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M9 18l6-6-6-6"/></svg>
			</a>
			<a href="/admin/tags" class="flex items-center justify-between px-4 py-3 hover:bg-slate-700 transition-colors">
				<span class="text-sm font-medium text-slate-100">Tags</span>
				<svg class="h-4 w-4 text-slate-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M9 18l6-6-6-6"/></svg>
			</a>
			<a href="/admin/container-types" class="flex items-center justify-between px-4 py-3 hover:bg-slate-700 transition-colors">
				<span class="text-sm font-medium text-slate-100">Container Types</span>
				<svg class="h-4 w-4 text-slate-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M9 18l6-6-6-6"/></svg>
			</a>
		</div>

		<!-- Session -->
		<div class="card">
			<button class="flex w-full items-center justify-between px-4 py-3 text-red-400 hover:bg-slate-700 transition-colors" on:click={logout}>
				<span class="text-sm font-medium">Sign out</span>
				<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" />
					<polyline points="16 17 21 12 16 7" />
					<line x1="21" y1="12" x2="9" y2="12" />
				</svg>
			</button>
		</div>
	</div>
</div>
