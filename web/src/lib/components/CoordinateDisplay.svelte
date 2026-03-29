<script lang="ts">
	import { parseCoordinate, parseLocationSchema, formatCoordinate } from '$lib/coordinate-helpers.js';

	let { coordinate = null, schema = null }: { coordinate?: unknown | null; schema?: unknown | null } = $props();

	let parsed = $derived(parseCoordinate(coordinate));
	let parsedSchema = $derived(parseLocationSchema(schema));
	let label = $derived(formatCoordinate(coordinate, schema));

	// Detect stale/out-of-bounds coordinates so callers get a visible hint.
	let isStale = $derived((() => {
		if (!parsed || !parsedSchema) return false;
		if (parsed.type !== parsedSchema.type) return true;
		if (parsed.type === 'abstract' && parsedSchema.type === 'abstract') {
			return !!(parsedSchema.labels && !parsedSchema.labels.includes(parsed.value));
		}
		if (parsed.type === 'grid' && parsedSchema.type === 'grid') {
			return parsed.row >= parsedSchema.rows || parsed.column >= parsedSchema.columns;
		}
		return false;
	})());
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
		<span class="text-sm {isStale ? 'text-amber-400' : 'text-slate-200'}" title={isStale ? 'This position no longer exists in the container\'s current schema' : undefined}>
			{label}{isStale ? ' (stale)' : ''}
		</span>
	{:else}
		<span class="text-xs font-mono text-slate-400">{label}</span>
	{/if}
{/if}
