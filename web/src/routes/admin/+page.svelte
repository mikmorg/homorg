<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { api } from '$api/client.js';
	import { isAdmin } from '$stores/auth.js';
	import type { StatsResponse } from '$api/types.js';
	import { toast } from '$stores/toast.js';

	let stats: StatsResponse | null = null;
	let loading = true;
	let statsError = '';
	let rebuilding = false;

	let labelCount = 30;
	let generatingLabels = false;

	async function downloadLabels() {
		if (labelCount < 1 || labelCount > 1000) {
			toast('Count must be between 1 and 1000', 'error');
			return;
		}
		generatingLabels = true;
		try {
			const blob = await api.barcodes.downloadLabels(labelCount);
			const url = URL.createObjectURL(blob);
			const a = document.createElement('a');
			a.href = url;
			a.download = `labels-${labelCount}.pdf`;
			a.click();
			URL.revokeObjectURL(url);
		} catch (err) {
			toast(err instanceof Error ? err.message : 'Label generation failed', 'error');
		} finally {
			generatingLabels = false;
		}
	}

	onMount(async () => {
		if (!$isAdmin) { goto('/'); return; }
		try {
			stats = await api.system.stats();
		} catch (err) {
			statsError = err instanceof Error ? err.message : 'Failed to load stats';
		} finally {
			loading = false;
		}
	});

	async function rebuildProjections() {
		if (!confirm('Rebuild all projections? This replays the entire event log and may take a while.')) return;
		rebuilding = true;
		try {
			await api.system.rebuildProjections();
			toast('Projections rebuild started', 'success');
		} catch (err) {
			toast(err instanceof Error ? err.message : 'Rebuild failed', 'error');
		} finally {
			rebuilding = false;
		}
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
		{:else if statsError}
			<div class="rounded-lg bg-red-950 px-4 py-3 text-sm text-red-300 border border-red-800">{statsError}</div>
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

		<!-- Print Labels -->
		<div class="card p-4 space-y-3">
			<p class="text-xs font-medium text-slate-400 uppercase tracking-wide">Print Labels</p>
			<p class="text-xs text-slate-500">Generates new barcodes and downloads a PDF (OL25WX, 3×10 per page).</p>
			<div class="flex items-center gap-3">
				<label class="text-sm text-slate-300 shrink-0" for="label-count">Labels</label>
				<input
					id="label-count"
					type="number"
					min="1"
					max="1000"
					step="30"
					bind:value={labelCount}
					class="w-24 rounded-md bg-slate-700 border border-slate-600 px-3 py-1.5 text-sm text-slate-100 focus:outline-none focus:ring-2 focus:ring-indigo-500"
				/>
				<button
					class="btn-primary flex items-center gap-2 disabled:opacity-50"
					onclick={downloadLabels}
					disabled={generatingLabels}
				>
					{#if generatingLabels}
						<div class="h-4 w-4 animate-spin rounded-full border-2 border-white/30 border-t-white"></div>
						Generating…
					{:else}
						<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
							<polyline points="7 10 12 15 17 10"/>
							<line x1="12" y1="15" x2="12" y2="3"/>
						</svg>
						Download PDF
					{/if}
				</button>
			</div>
		</div>

		<!-- System -->
		<div class="card divide-y divide-slate-700">
			<button
				class="flex w-full items-center justify-between px-4 py-3 hover:bg-slate-700 transition-colors"
				onclick={rebuildProjections}
				disabled={rebuilding}
			>
				<span class="text-sm font-medium text-slate-100">{rebuilding ? 'Rebuilding…' : 'Rebuild projections'}</span>
				<svg class="h-4 w-4 text-slate-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M1 4v6h6M23 20v-6h-6" />
					<path d="M20.49 9A9 9 0 0 0 5.64 5.64L1 10m22 4l-4.64 4.36A9 9 0 0 1 3.51 15" />
				</svg>
			</button>
		</div>


	</div>
</div>
