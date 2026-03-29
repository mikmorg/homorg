<script lang="ts">
	import { parseLocationSchema, schemaTypeLabel } from '$lib/coordinate-helpers.js';

	let { schema = null }: { schema?: unknown | null } = $props();

	let parsed = $derived(parseLocationSchema(schema));
	let typeLabel = $derived(schemaTypeLabel(schema));
</script>

{#if schema != null}
	<div class="text-sm text-slate-200">
		<span class="font-medium">{typeLabel}</span>

		{#if parsed?.type === 'abstract' && parsed.labels && parsed.labels.length > 0}
			<div class="mt-1 flex flex-wrap gap-1">
				{#each parsed.labels as lbl}
					<span class="badge text-xs">{lbl}</span>
				{/each}
			</div>
		{:else if parsed?.type === 'grid'}
			<span class="text-xs text-slate-400 ml-1">
				{parsed.rows} rows, {parsed.columns} columns
			</span>
			{#if parsed.row_labels}
				<div class="mt-1 text-xs text-slate-400">
					Rows: {parsed.row_labels.join(', ')}
				</div>
			{/if}
			{#if parsed.column_labels}
				<div class="text-xs text-slate-400">
					Cols: {parsed.column_labels.join(', ')}
				</div>
			{/if}
		{:else if parsed?.type === 'geo'}
			<span class="text-xs text-slate-400 ml-1">Latitude / Longitude</span>
		{/if}
	</div>
{:else}
	<span class="text-sm text-slate-500">No location schema</span>
{/if}
