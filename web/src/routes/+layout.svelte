<script lang="ts">
	import '../app.css';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';
	import { isAuthenticated, isAdmin } from '$stores/auth.js';
	import { pendingCount } from '$offline/queue.js';
	import { startHidScanner } from '$scanner/index.js';
	import { registerSyncListeners } from '$offline/queue.js';
	import { getAccessToken } from '$stores/auth.js';
	import Toast from '$lib/components/Toast.svelte';

	let { children } = $props();

	const PUBLIC_PATHS = ['/', '/login', '/setup', '/register'];

	onMount(() => {
		startHidScanner().catch(console.error);
		registerSyncListeners(getAccessToken);
	});

	$effect(() => {
		const path = page.url.pathname;
		if (!$isAuthenticated && !PUBLIC_PATHS.some((p) => path === p || (p !== '/' && path.startsWith(p)))) {
			goto('/login');
		}
	});

	let navPath = $derived(page.url.pathname);
	function isActive(prefix: string) {
		return navPath === prefix || navPath.startsWith(prefix + '/');
	}
</script>

<div class="flex h-dvh flex-col bg-slate-950 text-slate-100">
	<!-- Main content area -->
	<main class="flex-1 overflow-y-auto">
		{@render children()}
	</main>

	<!-- Bottom navigation — only show when authenticated -->
	{#if $isAuthenticated && !PUBLIC_PATHS.includes(navPath)}
		<nav
			class="flex shrink-0 border-t border-slate-800 bg-slate-900"
			style="padding-bottom: env(safe-area-inset-bottom)"
		>
			<a
				href="/stocker"
				class="nav-tab"
				class:nav-tab-active={isActive('/stocker')}
				aria-label="Stocker"
			>
				<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<rect x="2" y="7" width="20" height="14" rx="2" />
					<path d="M16 7V5a2 2 0 0 0-4 0v2" />
					<line x1="12" y1="12" x2="12" y2="16" />
					<line x1="10" y1="14" x2="14" y2="14" />
				</svg>
				<span class="text-xs">Stocker</span>
			</a>

			<a
				href="/browse"
				class="nav-tab"
				class:nav-tab-active={isActive('/browse')}
				aria-label="Browse"
			>
				<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<rect x="3" y="3" width="7" height="7" rx="1" />
					<rect x="14" y="3" width="7" height="7" rx="1" />
					<rect x="3" y="14" width="7" height="7" rx="1" />
					<rect x="14" y="14" width="7" height="7" rx="1" />
				</svg>
				<span class="text-xs">Browse</span>
			</a>

			<a
				href="/scan"
				class="nav-tab"
				class:nav-tab-active={isActive('/scan')}
				aria-label="Scan"
			>
				<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M3 7V5a2 2 0 0 1 2-2h2M17 3h2a2 2 0 0 1 2 2v2M21 17v2a2 2 0 0 1-2 2h-2M7 21H5a2 2 0 0 1-2-2v-2" />
					<line x1="7" y1="12" x2="7" y2="12.01" />
					<line x1="12" y1="12" x2="17" y2="12" />
					<line x1="12" y1="16" x2="17" y2="16" />
					<line x1="7" y1="16" x2="7" y2="16.01" />
					<line x1="7" y1="8" x2="17" y2="8" />
				</svg>
				<span class="text-xs">Scan</span>
			</a>

			<a
				href="/search"
				class="nav-tab"
				class:nav-tab-active={isActive('/search')}
				aria-label="Search"
			>
				<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<circle cx="11" cy="11" r="8" />
					<path d="m21 21-4.35-4.35" />
				</svg>
				<span class="text-xs">Search</span>
			</a>

			{#if $isAdmin}
				<a
					href="/admin"
					class="nav-tab"
					class:nav-tab-active={isActive('/admin')}
					aria-label="Admin"
				>
					<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
						<path d="M12 2L2 7l10 5 10-5-10-5z" />
						<path d="M2 17l10 5 10-5M2 12l10 5 10-5" />
					</svg>
					<span class="text-xs">Admin</span>
				</a>
			{/if}

			<a
				href="/account"
				class="nav-tab"
				class:nav-tab-active={isActive('/account')}
				aria-label="Account"
			>
				<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
					<circle cx="12" cy="7" r="4" />
				</svg>
				<span class="text-xs">Account</span>
			</a>
		</nav>
	{/if}

	<!-- Offline indicator -->
	{#if $pendingCount > 0}
		<div class="fixed bottom-20 left-1/2 -translate-x-1/2 z-50">
			<div class="rounded-full bg-amber-600 px-3 py-1 text-xs font-medium text-white shadow-lg">
				{$pendingCount} pending sync
			</div>
		</div>
	{/if}
<Toast />
</div>

<style>
	.nav-tab {
		display: flex;
		flex: 1 1 0%;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 0.25rem;
		padding-top: 0.5rem;
		padding-bottom: 0.5rem;
		color: rgb(100 116 139);
		transition-property: color;
		transition-timing-function: cubic-bezier(0.4, 0, 0.2, 1);
		transition-duration: 150ms;
		min-height: 56px;
	}
	.nav-tab-active {
		color: rgb(129 140 248);
	}
</style>
