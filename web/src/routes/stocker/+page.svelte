<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { api } from '$api/client.js';
	import type { ScanSession } from '$api/types.js';

	let sessions: ScanSession[] = [];
	let loading = true;
	let error = '';
	let creating = false;

	onMount(async () => {
		await loadSessions();
	});

	async function loadSessions() {
		loading = true;
		error = '';
		try {
			sessions = await api.stocker.listSessions();
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to load sessions';
		} finally {
			loading = false;
		}
	}

	async function startSession() {
		creating = true;
		error = '';
		try {
			const session = await api.stocker.startSession({ notes: undefined });
			goto(`/stocker/${session.id}`);
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to start session';
			creating = false;
		}
	}

	function formatDate(iso: string) {
		return new Date(iso).toLocaleString(undefined, {
			month: 'short',
			day: 'numeric',
			hour: '2-digit',
			minute: '2-digit'
		});
	}
</script>

<svelte:head>
	<title>Stocker — Homorg</title>
</svelte:head>

<div class="flex h-full flex-col">
	<!-- Header -->
	<header class="flex items-center justify-between border-b border-slate-800 px-4 py-3">
		<h1 class="text-lg font-semibold text-slate-100">Stocker</h1>
		<button class="btn btn-primary" on:click={startSession} disabled={creating}>
			{#if creating}
				<span class="h-4 w-4 animate-spin rounded-full border-2 border-white/30 border-t-white"></span>
			{:else}
				New session
			{/if}
		</button>
	</header>

	<div class="flex-1 overflow-y-auto p-4">
		{#if error}
			<div class="mb-4 rounded-lg bg-red-950 px-4 py-3 text-sm text-red-300 border border-red-800">
				{error}
			</div>
		{/if}

		{#if loading}
			<div class="flex h-32 items-center justify-center">
				<div class="h-6 w-6 animate-spin rounded-full border-2 border-slate-600 border-t-indigo-500"></div>
			</div>
		{:else if sessions.length === 0}
			<div class="flex h-40 flex-col items-center justify-center gap-2 text-slate-500">
				<svg class="h-10 w-10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
					<rect x="2" y="7" width="20" height="14" rx="2" />
					<path d="M16 7V5a2 2 0 0 0-4 0v2" />
				</svg>
				<p class="text-sm">No sessions yet</p>
				<button class="btn btn-secondary mt-2" on:click={startSession}>
					Start your first session
				</button>
			</div>
		{:else}
			<div class="space-y-2">
				{#each sessions as session (session.id)}
					<a
						href="/stocker/{session.id}"
						class="card flex items-center gap-3 p-4 transition-colors hover:bg-slate-700"
					>
						<!-- Status dot -->
						<div class="relative flex-shrink-0">
							<div
								class="h-3 w-3 rounded-full"
								class:bg-green-400={session.ended_at === null}
								class:bg-slate-600={session.ended_at !== null}
							></div>
							{#if session.ended_at === null}
								<div class="absolute inset-0 h-3 w-3 animate-ping rounded-full bg-green-400 opacity-60"></div>
							{/if}
						</div>

						<div class="min-w-0 flex-1">
							<div class="flex items-center gap-2">
								<span class="font-medium text-slate-100 truncate">
									{session.ended_at === null ? 'Active session' : 'Session'}
								</span>
								{#if session.ended_at === null}
									<span class="badge bg-green-900 text-green-300">live</span>
								{/if}
							</div>
							<p class="mt-0.5 text-xs text-slate-400">
								Started {formatDate(session.started_at)}
								{#if session.ended_at}
									· Ended {formatDate(session.ended_at)}
								{/if}
							</p>
							{#if session.notes}
								<p class="mt-1 text-sm text-slate-300 truncate">{session.notes}</p>
							{/if}
						</div>

						<svg class="h-4 w-4 flex-shrink-0 text-slate-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M9 18l6-6-6-6" />
						</svg>
					</a>
				{/each}
			</div>
		{/if}
	</div>
</div>
