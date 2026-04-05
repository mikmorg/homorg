<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { api } from '$api/client.js';
	import { isAdmin } from '$stores/auth.js';
	import type { StatsResponse, ContainerType } from '$api/types.js';
	import { toast } from '$stores/toast.js';

	let stats: StatsResponse | null = $state(null);
	let loading = $state(true);
	let statsError = $state('');
	let rebuilding = $state(false);

	let labelCount = $state(30);
	let generatingLabels = $state(false);

	// Preset labels
	let containerTypes: ContainerType[] = $state([]);
	let presetItemCount = $state(30);
	let presetContainerCount = $state(30);
	let presetContainerTypeId = $state('');
	let generatingPresetItem = $state(false);
	let generatingPresetContainer = $state(false);

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

	async function downloadPresetItemLabels() {
		if (presetItemCount < 1 || presetItemCount > 1000) {
			toast('Count must be between 1 and 1000', 'error');
			return;
		}
		generatingPresetItem = true;
		try {
			const blob = await api.barcodes.downloadPresetLabels(presetItemCount, false);
			const url = URL.createObjectURL(blob);
			const a = document.createElement('a');
			a.href = url;
			a.download = `preset-item-labels-${presetItemCount}.pdf`;
			a.click();
			URL.revokeObjectURL(url);
		} catch (err) {
			toast(err instanceof Error ? err.message : 'Label generation failed', 'error');
		} finally {
			generatingPresetItem = false;
		}
	}

	async function downloadPresetContainerLabels() {
		if (presetContainerCount < 1 || presetContainerCount > 1000) {
			toast('Count must be between 1 and 1000', 'error');
			return;
		}
		generatingPresetContainer = true;
		try {
			const blob = await api.barcodes.downloadPresetLabels(
				presetContainerCount,
				true,
				presetContainerTypeId || undefined
			);
			const url = URL.createObjectURL(blob);
			const a = document.createElement('a');
			a.href = url;
			a.download = `preset-container-labels-${presetContainerCount}.pdf`;
			a.click();
			URL.revokeObjectURL(url);
		} catch (err) {
			toast(err instanceof Error ? err.message : 'Label generation failed', 'error');
		} finally {
			generatingPresetContainer = false;
		}
	}

	onMount(async () => {
		if (!$isAdmin) { goto('/'); return; }
		try {
			[stats, containerTypes] = await Promise.all([
				api.system.stats(),
				api.containerTypes.list()
			]);
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
			<p class="text-xs font-medium text-slate-400 uppercase tracking-wide">Blank Labels</p>
			<p class="text-xs text-slate-500">Unregistered barcodes for assigning to existing items. When scanned in a stocker session you'll be prompted to name the item manually.</p>
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

		<!-- Print Preset Labels -->
		<div class="card p-4 space-y-4">
			<div>
				<p class="text-xs font-medium text-slate-400 uppercase tracking-wide">Scan-to-Create Labels</p>
				<p class="text-xs text-slate-500 mt-1">Pre-registered barcodes. Scanning one in a stocker session instantly creates the record — no name prompt, barcode becomes the default name.</p>
			</div>

			<!-- Item preset labels -->
			<div class="space-y-2">
				<p class="text-xs text-slate-400 font-medium">Items</p>
				<div class="flex items-center gap-3">
					<label class="text-sm text-slate-300 shrink-0" for="preset-item-count">Count</label>
					<input
						id="preset-item-count"
						type="number"
						min="1"
						max="1000"
						step="10"
						bind:value={presetItemCount}
						class="w-24 rounded-md bg-slate-700 border border-slate-600 px-3 py-1.5 text-sm text-slate-100 focus:outline-none focus:ring-2 focus:ring-indigo-500"
					/>
					<button
						class="btn-primary flex items-center gap-2 disabled:opacity-50"
						onclick={downloadPresetItemLabels}
						disabled={generatingPresetItem}
					>
						{#if generatingPresetItem}
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

			<!-- Container preset labels -->
			<div class="space-y-2">
				<p class="text-xs text-slate-400 font-medium">Containers</p>
				<div class="flex flex-col gap-2">
					<div class="flex items-center gap-3">
						<label class="text-sm text-slate-300 shrink-0" for="preset-container-type">Type</label>
						<select
							id="preset-container-type"
							bind:value={presetContainerTypeId}
							class="flex-1 rounded-md bg-slate-700 border border-slate-600 px-3 py-1.5 text-sm text-slate-100 focus:outline-none focus:ring-2 focus:ring-indigo-500"
						>
							<option value="">— No type —</option>
							{#each containerTypes as ct}
								<option value={ct.id}>{ct.name}</option>
							{/each}
						</select>
					</div>
					<div class="flex items-center gap-3">
						<label class="text-sm text-slate-300 shrink-0" for="preset-container-count">Count</label>
						<input
							id="preset-container-count"
							type="number"
							min="1"
							max="1000"
							step="10"
							bind:value={presetContainerCount}
							class="w-24 rounded-md bg-slate-700 border border-slate-600 px-3 py-1.5 text-sm text-slate-100 focus:outline-none focus:ring-2 focus:ring-indigo-500"
						/>
						<button
							class="btn-primary flex items-center gap-2 disabled:opacity-50"
							onclick={downloadPresetContainerLabels}
							disabled={generatingPresetContainer}
						>
							{#if generatingPresetContainer}
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
