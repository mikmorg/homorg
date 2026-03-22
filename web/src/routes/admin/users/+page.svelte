<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { api } from '$api/client.js';
	import { isAdmin, currentUser } from '$stores/auth.js';
	import type { UserPublic, InviteResponse } from '$api/types.js';

	let users: UserPublic[] = [];
	let loading = true;
	let error = '';

	// Invite
	let inviteCode: string | null = null;
	let inviteLoading = false;

	// Role change
	let changingRole: string | null = null;

	const ROLE_LABELS: Record<string, string> = {
		admin: 'Admin',
		member: 'Member',
		readonly: 'Read-only'
	};

	const ROLE_COLORS: Record<string, string> = {
		admin: 'bg-indigo-900 text-indigo-300',
		member: 'bg-emerald-900 text-emerald-300',
		readonly: 'bg-slate-700 text-slate-300'
	};

	onMount(async () => {
		if (!$isAdmin) { goto('/'); return; }
		await loadUsers();
	});

	async function loadUsers() {
		loading = true;
		error = '';
		try {
			users = await api.users.list();
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to load users';
		} finally {
			loading = false;
		}
	}

	async function changeRole(userId: string, newRole: 'admin' | 'member' | 'readonly') {
		changingRole = userId;
		try {
			await api.users.updateRole(userId, { role: newRole });
			await loadUsers();
		} catch (err) {
			alert(err instanceof Error ? err.message : 'Failed to change role');
		} finally {
			changingRole = null;
		}
	}

	async function deactivateUser(user: UserPublic) {
		if (!confirm(`Deactivate ${user.username}? They will no longer be able to log in.`)) return;
		try {
			await api.users.deactivate(user.id);
			await loadUsers();
		} catch (err) {
			alert(err instanceof Error ? err.message : 'Failed to deactivate');
		}
	}

	async function createInvite() {
		inviteLoading = true;
		try {
			const resp: InviteResponse = await api.auth.invite();
			inviteCode = resp.code;
		} catch (err) {
			alert(err instanceof Error ? err.message : 'Failed to create invite');
		} finally {
			inviteLoading = false;
		}
	}

	function copyInvite() {
		if (inviteCode) navigator.clipboard.writeText(inviteCode);
	}

	function formatDate(iso: string) {
		return new Date(iso).toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' });
	}
</script>

<svelte:head>
	<title>Users — Homorg</title>
</svelte:head>

<div class="flex h-full flex-col">
	<header class="flex items-center gap-2 border-b border-slate-800 px-3 py-2">
		<a href="/admin" class="btn btn-icon text-slate-400">
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M15 18l-6-6 6-6" />
			</svg>
		</a>
		<h1 class="flex-1 text-base font-semibold text-slate-100">Users</h1>
		<button class="btn btn-primary text-xs" on:click={createInvite} disabled={inviteLoading}>
			{inviteLoading ? '…' : 'Invite'}
		</button>
	</header>

	<!-- Invite code banner -->
	{#if inviteCode}
		<div class="border-b border-slate-800 bg-indigo-950 px-4 py-3">
			<p class="text-xs text-indigo-300 mb-1">Share this invite code:</p>
			<div class="flex items-center gap-2">
				<code class="flex-1 rounded bg-slate-800 px-3 py-1.5 font-mono text-sm text-slate-100">{inviteCode}</code>
				<button class="btn btn-secondary text-xs px-2 py-1" on:click={copyInvite}>Copy</button>
				<button class="btn btn-ghost text-xs px-2 py-1 text-slate-400" on:click={() => (inviteCode = null)}>Dismiss</button>
			</div>
		</div>
	{/if}

	<div class="flex-1 overflow-y-auto">
		{#if loading}
			<div class="flex h-32 items-center justify-center">
				<div class="h-6 w-6 animate-spin rounded-full border-2 border-slate-600 border-t-indigo-500"></div>
			</div>
		{:else if error}
			<div class="m-4 rounded-lg bg-red-950 px-4 py-3 text-sm text-red-300 border border-red-800">{error}</div>
		{:else if users.length === 0}
			<div class="flex h-32 items-center justify-center text-sm text-slate-500">No users</div>
		{:else}
			<div class="divide-y divide-slate-800">
				{#each users as user (user.id)}
					<div class="px-4 py-3 space-y-2">
						<div class="flex items-center gap-3">
							<div class="flex-1 min-w-0">
								<div class="flex items-center gap-2">
									<span class="font-medium text-slate-100 truncate">{user.display_name ?? user.username}</span>
									{#if !user.is_active}
										<span class="rounded-full bg-red-900 px-2 py-0.5 text-xs text-red-300">Inactive</span>
									{/if}
								</div>
								<p class="text-xs text-slate-400">@{user.username} · Joined {formatDate(user.created_at)}</p>
							</div>
							<span class="rounded-full px-2.5 py-0.5 text-xs font-medium {ROLE_COLORS[user.role] ?? ROLE_COLORS.readonly}">
								{ROLE_LABELS[user.role] ?? user.role}
							</span>
						</div>

						{#if user.is_active && user.id !== $currentUser?.id}
							<div class="flex items-center gap-2">
								<select
									class="input text-xs py-1"
									value={user.role}
									on:change={(e) => changeRole(user.id, e.currentTarget.value as 'admin' | 'member' | 'readonly')}
									disabled={changingRole === user.id}
								>
									<option value="admin">Admin</option>
									<option value="member">Member</option>
									<option value="readonly">Read-only</option>
								</select>
								<button class="btn btn-ghost text-xs text-red-400 px-2 py-1" on:click={() => deactivateUser(user)}>
									Deactivate
								</button>
							</div>
						{/if}
					</div>
				{/each}
			</div>
		{/if}
	</div>
</div>
