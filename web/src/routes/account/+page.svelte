<script lang="ts">
	import { goto } from '$app/navigation';
	import { api } from '$api/client.js';
	import { authStore, currentUser } from '$stores/auth.js';
	import { toast } from '$stores/toast.js';
	import { get } from 'svelte/store';

	let loggingOut = false;

	// Profile editing
	let editingProfile = false;
	let displayName = '';
	let newPassword = '';
	let confirmPassword = '';
	let savingProfile = false;

	function startEdit() {
		const user = get(currentUser);
		displayName = user?.display_name ?? '';
		newPassword = '';
		confirmPassword = '';
		editingProfile = true;
	}

	async function saveProfile() {
		if (newPassword && newPassword !== confirmPassword) {
			toast('Passwords do not match', 'error');
			return;
		}
		savingProfile = true;
		try {
			const user = get(currentUser);
			if (!user) return;
			const updates: Record<string, string> = {};
			if (displayName !== (user.display_name ?? '')) updates.display_name = displayName;
			if (newPassword) updates.password = newPassword;
			if (Object.keys(updates).length === 0) {
				editingProfile = false;
				savingProfile = false;
				return;
			}
			await api.users.update(user.id, updates);
			if (updates.display_name !== undefined) {
				authStore.updateUser({ ...user, display_name: displayName || null });
			}
			editingProfile = false;
			toast('Profile updated', 'success');
		} catch (err) {
			toast(err instanceof Error ? err.message : 'Update failed', 'error');
		} finally {
			savingProfile = false;
		}
	}

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
						{#if !editingProfile}
							<button class="text-xs text-indigo-400 hover:text-indigo-300" on:click={startEdit}>Edit</button>
						{/if}
					</div>
				</div>

				{#if editingProfile}
					<div class="card p-4 space-y-3">
						<div>
							<label class="mb-1 block text-sm font-medium text-slate-300" for="acct-name">Display name</label>
							<input id="acct-name" class="input" bind:value={displayName} placeholder="Display name" />
						</div>
						<div>
							<label class="mb-1 block text-sm font-medium text-slate-300" for="acct-pw">New password</label>
							<input id="acct-pw" class="input" type="password" autocomplete="new-password" bind:value={newPassword} placeholder="Leave blank to keep current" />
						</div>
						{#if newPassword}
							<div>
								<label class="mb-1 block text-sm font-medium text-slate-300" for="acct-pw2">Confirm password</label>
								<input id="acct-pw2" class="input" type="password" autocomplete="new-password" bind:value={confirmPassword} />
							</div>
						{/if}
						<div class="flex gap-2">
							<button class="btn btn-primary flex-1" on:click={saveProfile} disabled={savingProfile}>
								{savingProfile ? 'Saving…' : 'Save'}
							</button>
							<button class="btn btn-secondary flex-1" on:click={() => { editingProfile = false; }}>Cancel</button>
						</div>
					</div>
				{/if}
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
