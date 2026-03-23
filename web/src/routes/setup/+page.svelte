<script lang="ts">
	import { goto } from '$app/navigation';
	import { api } from '$api/client.js';
	import { authStore } from '$stores/auth.js';

	let username = '';
	let password = '';
	let passwordConfirm = '';
	let loading = false;
	let error = '';

	async function setup(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		if (password !== passwordConfirm) {
			error = 'Passwords do not match.';
			return;
		}
		loading = true;
		try {
			const res = await api.auth.setup({ username, password });
			authStore.set(res);
			goto('/browse');
		} catch (err) {
			error = err instanceof Error ? err.message : 'Setup failed';
		} finally {
			loading = false;
		}
	}
</script>

<svelte:head>
	<title>Initial Setup — Homorg</title>
</svelte:head>

<div class="flex min-h-dvh flex-col items-center justify-center px-4">
	<div class="w-full max-w-sm">
		<div class="mb-8 text-center">
			<h1 class="text-2xl font-bold tracking-tight text-slate-100">Welcome to Homorg</h1>
			<p class="mt-1 text-sm text-slate-400">Create your administrator account to get started.</p>
		</div>

		<form class="space-y-4" on:submit={setup}>
			{#if error}
				<div class="rounded-lg bg-red-950 px-4 py-3 text-sm text-red-300 border border-red-800">
					{error}
				</div>
			{/if}

			<div>
				<label class="mb-1.5 block text-sm font-medium text-slate-300" for="username">
					Admin username
				</label>
				<input
					id="username"
					class="input"
					type="text"
					autocomplete="username"
					autocapitalize="none"
					bind:value={username}
					required
					disabled={loading}
				/>
			</div>

			<div>
				<label class="mb-1.5 block text-sm font-medium text-slate-300" for="password">
					Password
				</label>
				<input
					id="password"
					class="input"
					type="password"
					autocomplete="new-password"
					bind:value={password}
					required
					disabled={loading}
				/>
			</div>

			<div>
				<label class="mb-1.5 block text-sm font-medium text-slate-300" for="password-confirm">
					Confirm password
				</label>
				<input
					id="password-confirm"
					class="input"
					type="password"
					autocomplete="new-password"
					bind:value={passwordConfirm}
					required
					disabled={loading}
				/>
			</div>

			<button type="submit" class="btn btn-primary w-full" disabled={loading}>
				{#if loading}
					<span class="h-4 w-4 animate-spin rounded-full border-2 border-white/30 border-t-white"></span>
				{:else}
					Create account
				{/if}
			</button>
		</form>
	</div>
</div>
