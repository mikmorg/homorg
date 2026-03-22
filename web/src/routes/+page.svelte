<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { get } from 'svelte/store';
	import { isAuthenticated } from '$stores/auth.js';
	import { api } from '$api/client.js';

	onMount(async () => {
		// Check if the system needs initial setup
		try {
			const health = await api.system.health();
			if (health.setup_required) {
				goto('/setup');
				return;
			}
		} catch (err) {
			// If the backend is completely unreachable, don't silently redirect to login.
			// Re-check after a brief delay — connection may still be starting up.
			const msg = err instanceof Error ? err.message : String(err);
			if (msg.includes('fetch') || msg.includes('network') || msg.includes('Failed')) {
				await new Promise((r) => setTimeout(r, 2000));
				try {
					const health2 = await api.system.health();
					if (health2.setup_required) { goto('/setup'); return; }
				} catch {
					goto('/login');
					return;
				}
			}
		}

		if (get(isAuthenticated)) {
			goto('/browse');
		} else {
			goto('/login');
		}
	});
</script>

<div class="flex h-dvh items-center justify-center">
	<div class="h-8 w-8 animate-spin rounded-full border-2 border-slate-600 border-t-indigo-500"></div>
</div>
