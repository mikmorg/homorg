<script lang="ts">
	import type { AncestorEntry } from '$api/types.js';

	export let ancestors: AncestorEntry[] = [];
	export let rootLabel = 'Root';
</script>

{#if ancestors.length > 0}
	<div class="flex items-center gap-1 overflow-x-auto text-xs text-slate-400">
		<a href="/browse" class="flex-shrink-0 hover:text-slate-200">{rootLabel}</a>
		{#each ancestors as crumb, i}
			<svg class="h-3 w-3 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M9 18l6-6-6-6" />
			</svg>
			{#if i < ancestors.length - 1}
				<a href="/browse?id={crumb.id}" class="flex-shrink-0 hover:text-slate-200 truncate max-w-24">{crumb.name ?? 'Container'}</a>
			{:else}
				<span class="flex-shrink-0 text-slate-200 truncate max-w-32">{crumb.name ?? 'Container'}</span>
			{/if}
		{/each}
	</div>
{/if}
