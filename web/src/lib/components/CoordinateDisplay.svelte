<script lang="ts">
	import { parseCoordinate, formatCoordinate } from '$lib/coordinate-helpers.js';

	export let coordinate: unknown | null = null;
	export let schema: unknown | null = null;

	$: parsed = parseCoordinate(coordinate);
	$: label = formatCoordinate(coordinate, schema);
</script>

{#if coordinate != null}
	{#if parsed?.type === 'geo'}
		<a
			href="https://www.openstreetmap.org/?mlat={parsed.latitude}&mlon={parsed.longitude}#map=17/{parsed.latitude}/{parsed.longitude}"
			target="_blank"
			rel="noopener noreferrer"
			class="inline-flex items-center gap-1 text-sm text-indigo-400 hover:underline"
		>
			<svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M21 10c0 7-9 13-9 13s-9-6-9-13a9 9 0 0 1 18 0z" />
				<circle cx="12" cy="10" r="3" />
			</svg>
			{label}
		</a>
	{:else if parsed}
		<span class="text-sm text-slate-200">{label}</span>
	{:else}
		<span class="text-xs font-mono text-slate-400">{label}</span>
	{/if}
{/if}
