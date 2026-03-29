<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { api } from '$api/client.js';
	import { authStore } from '$stores/auth.js';

	let username = $state('');
	let password = $state('');
	let displayName = $state('');
	let inviteCode = $state('');
	let confirmPassword = $state('');
	let loading = $state(false);
	let error = $state('');

	// Pre-fill invite code from URL if provided
	$effect(() => {
		const code = page.url.searchParams.get('code');
		if (code && !inviteCode) inviteCode = code;
	});

	async function register(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		if (password !== confirmPassword) { error = 'Passwords do not match'; return; }
		loading = true;
		try {
			const res = await api.auth.register({
				username,
				password,
				invite_code: inviteCode,
				display_name: displayName || undefined
			});
			authStore.set(res);
			goto('/browse');
		} catch (err) {
			error = err instanceof Error ? err.message : 'Registration failed';
		} finally {
			loading = false;
		}
	}
</script>

<svelte:head>
	<title>Register — Homorg</title>
</svelte:head>

<div class="flex min-h-dvh flex-col items-center justify-center px-4">
	<div class="w-full max-w-sm">
		<div class="mb-8 text-center">
			<h1 class="text-2xl font-bold tracking-tight text-slate-100">Homorg</h1>
			<p class="mt-1 text-sm text-slate-400">Create your account</p>
		</div>

		<form class="space-y-4" onsubmit={register}>
			{#if error}
				<div class="rounded-lg bg-red-950 px-4 py-3 text-sm text-red-300 border border-red-800">
					{error}
				</div>
			{/if}

			<div>
				<label class="mb-1.5 block text-sm font-medium text-slate-300" for="invite-code">
					Invite code
				</label>
				<input
					id="invite-code"
					class="input"
					type="text"
					bind:value={inviteCode}
					placeholder="Paste your invite code"
					required
					disabled={loading}
				/>
			</div>

			<div>
				<label class="mb-1.5 block text-sm font-medium text-slate-300" for="reg-username">
					Username
				</label>
				<input
					id="reg-username"
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
				<label class="mb-1.5 block text-sm font-medium text-slate-300" for="display-name">
					Display name
				</label>
				<input
					id="display-name"
					class="input"
					type="text"
					bind:value={displayName}
					placeholder="Optional"
					disabled={loading}
				/>
			</div>

			<div>
				<label class="mb-1.5 block text-sm font-medium text-slate-300" for="reg-password">
					Password
				</label>
				<input
					id="reg-password"
					class="input"
					type="password"
					autocomplete="new-password"
					bind:value={password}
					required
					disabled={loading}
				/>
			</div>

			<div>
				<label class="mb-1.5 block text-sm font-medium text-slate-300" for="reg-confirm">
					Confirm password
				</label>
				<input
					id="reg-confirm"
					class="input"
					type="password"
					autocomplete="new-password"
					bind:value={confirmPassword}
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

		<p class="mt-6 text-center text-sm text-slate-400">
			Already have an account?
			<a href="/login" class="text-indigo-400 hover:text-indigo-300">Sign in</a>
		</p>
	</div>
</div>
