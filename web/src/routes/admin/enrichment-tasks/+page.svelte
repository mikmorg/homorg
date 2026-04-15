<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { goto } from '$app/navigation';
	import { api } from '$api/client.js';
	import { isAdmin } from '$stores/auth.js';
	import { toast } from '$stores/toast.js';
	import type { EnrichmentTask, EnrichmentStatus } from '$api/types.js';

	let tasks: EnrichmentTask[] = $state([]);
	let loading = $state(true);
	let error = $state('');
	let busyId: string | null = $state(null);
	let statusFilter: EnrichmentStatus | '' = $state('');
	let timer: ReturnType<typeof setInterval> | null = null;

	const STATUS_COLORS: Record<EnrichmentStatus, string> = {
		pending: 'bg-slate-700 text-slate-300',
		in_progress: 'bg-indigo-900 text-indigo-300',
		succeeded: 'bg-emerald-900 text-emerald-300',
		failed: 'bg-yellow-900 text-yellow-300',
		dead: 'bg-red-900 text-red-300',
		canceled: 'bg-slate-800 text-slate-400'
	};

	onMount(async () => {
		if (!$isAdmin) {
			goto('/');
			return;
		}
		await load();
		// Auto-refresh every 5s while visible.
		timer = setInterval(() => {
			if (!document.hidden) load(false);
		}, 5000);
	});

	onDestroy(() => {
		if (timer) clearInterval(timer);
	});

	async function load(showSpinner = true) {
		if (showSpinner) loading = true;
		error = '';
		try {
			const params: { status?: EnrichmentStatus; limit: number } = { limit: 100 };
			if (statusFilter) params.status = statusFilter;
			tasks = await api.enrichment.listTasks(params);
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to load tasks';
		} finally {
			loading = false;
		}
	}

	async function retry(task: EnrichmentTask) {
		busyId = task.id;
		try {
			await api.enrichment.retryTask(task.id);
			toast('Task re-queued', 'success');
			await load(false);
		} catch (err) {
			toast(err instanceof Error ? err.message : 'Retry failed', 'error');
		} finally {
			busyId = null;
		}
	}

	async function cancel(task: EnrichmentTask) {
		busyId = task.id;
		try {
			await api.enrichment.cancelTask(task.id);
			toast('Task canceled', 'success');
			await load(false);
		} catch (err) {
			toast(err instanceof Error ? err.message : 'Cancel failed', 'error');
		} finally {
			busyId = null;
		}
	}

	function fmtTime(s: string | null): string {
		if (!s) return '—';
		const d = new Date(s);
		return d.toLocaleString();
	}
</script>

<svelte:head>
	<title>Enrichment tasks — Homorg</title>
</svelte:head>

<div class="flex h-full flex-col">
	<header class="flex items-center justify-between border-b border-slate-800 px-4 py-3">
		<h1 class="text-lg font-semibold text-slate-100">Enrichment tasks</h1>
		<a href="/admin" class="text-sm text-indigo-400 hover:text-indigo-300">← Admin</a>
	</header>

	<div class="flex-1 overflow-y-auto p-4 space-y-3">
		<div class="flex items-center gap-3">
			<label for="status-filter" class="text-sm text-slate-300">Status</label>
			<select
				id="status-filter"
				bind:value={statusFilter}
				onchange={() => load()}
				class="rounded-md bg-slate-700 border border-slate-600 px-3 py-1.5 text-sm text-slate-100 focus:outline-none focus:ring-2 focus:ring-indigo-500"
			>
				<option value="">All</option>
				<option value="pending">Pending</option>
				<option value="in_progress">In progress</option>
				<option value="succeeded">Succeeded</option>
				<option value="failed">Failed</option>
				<option value="dead">Dead</option>
				<option value="canceled">Canceled</option>
			</select>
			<span class="text-xs text-slate-500 ml-auto">Auto-refreshes every 5s</span>
		</div>

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
		{:else if tasks.length === 0}
			<p class="text-center text-sm text-slate-400 py-8">No tasks match this filter.</p>
		{:else}
			<div class="card divide-y divide-slate-800">
				{#each tasks as t (t.id)}
					<div class="p-3 space-y-2 text-sm">
						<div class="flex items-center gap-2">
							<span
								class="text-xs px-2 py-0.5 rounded-full {STATUS_COLORS[t.status] ??
									'bg-slate-700'}"
							>
								{t.status}
							</span>
							<span class="text-xs text-slate-500">{t.trigger_event}</span>
							<span class="text-xs text-slate-500">attempts {t.attempts}/{t.max_attempts}</span>
							<a
								href="/browse/item/{t.item_id}"
								class="ml-auto text-xs text-indigo-400 hover:text-indigo-300"
							>
								item →
							</a>
						</div>
						<div class="flex items-center gap-4 text-xs text-slate-400">
							<span>created: {fmtTime(t.created_at)}</span>
							{#if t.completed_at}
								<span>completed: {fmtTime(t.completed_at)}</span>
							{/if}
							{#if t.claimed_by}
								<span>claimed by: {t.claimed_by}</span>
							{/if}
						</div>
						{#if t.last_error}
							<p class="text-xs text-red-400 font-mono bg-slate-900 p-2 rounded">{t.last_error}</p>
						{/if}
						{#if t.status === 'failed' || t.status === 'dead' || t.status === 'canceled'}
							<div class="flex gap-2">
								<button
									class="text-xs text-indigo-400 hover:text-indigo-300 disabled:opacity-50"
									onclick={() => retry(t)}
									disabled={busyId === t.id}
								>
									Retry
								</button>
							</div>
						{:else if t.status === 'pending' || t.status === 'in_progress'}
							<div class="flex gap-2">
								<button
									class="text-xs text-slate-400 hover:text-slate-200 disabled:opacity-50"
									onclick={() => cancel(t)}
									disabled={busyId === t.id}
								>
									Cancel
								</button>
							</div>
						{/if}
					</div>
				{/each}
			</div>
		{/if}
	</div>
</div>
