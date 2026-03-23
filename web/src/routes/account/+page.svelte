<script lang="ts">
	import { goto } from '$app/navigation';
	import { api } from '$api/client.js';
	import { authStore, currentUser } from '$stores/auth.js';
	import { get } from 'svelte/store';

	let loggingOut = false;

	async function logout() {
		loggingOut = true;
		const auth = get(authStore);
		try { if (auth?.refresh_token) await api.auth.logout(auth.refresh_token); } catch { /* ignore */ }
		authStore.clear();
		goto('/login');
	}
</script>

<svelte:head>
	<title>Account — Homorg</title>
</svelte:head>

<div class="flex h-full flex-col">
	<header class="flex items-center gap-2 border-b border-slate-800 px-3 py-2">
		<h1 class="flex-1 text-base font-semibold text-slate-100">Account</h1>
	</header>

	<div class="flex-1 overflow-y-auto p-4">
		<div class="space-y-4">
			{#if $currentUser}
				<div class="card p-4">
					<div class="flex items-center gap-3">
						<div class="flex h-12 w-12 items-center justify-center rounded-full bg-indigo-500/20 text-lg text-indigo-400">
							{($currentUser.display_name ?? $currentUser.username ?? '?').charAt(0).toUpperCase()}
						</div>
						<div class="min-w-0 flex-1">
							<p class="font-medium text-slate-100 truncate">{$currentUser.display_name ?? $currentUser.username}</p>
							<p class="text-sm text-slate-400 truncate">{$currentUser.username}</p>
							<p class="text-xs text-slate-500 capitalize">{$currentUser.role}</p>
						</div>
					</div>
				</div>
			{/if}

			<button
				class="btn btn-danger w-full"
				on:click={logout}
				disabled={loggingOut}
			>
				{loggingOut ? 'Signing out…' : 'Sign out'}
			</button>
		</div>
	</div>
</div>
