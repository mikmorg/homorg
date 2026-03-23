<script lang="ts">
	import { parseLocationSchema } from '$lib/coordinate-helpers.js';

	export let value: unknown | null = null;

	let schemaType: '' | 'abstract' | 'grid' | 'geo' = '';
	let labels = '';
	let rows = 3;
	let columns = 3;
	let rowLabels = '';
	let columnLabels = '';

	// Initialize from existing value
	$: {
		const parsed = parseLocationSchema(value);
		if (parsed) {
			schemaType = parsed.type;
			if (parsed.type === 'abstract') {
				labels = parsed.labels?.join(', ') ?? '';
			} else if (parsed.type === 'grid') {
				rows = parsed.rows;
				columns = parsed.columns;
				rowLabels = parsed.row_labels?.join(', ') ?? '';
				columnLabels = parsed.column_labels?.join(', ') ?? '';
			}
		} else {
			schemaType = value ? '' : '';
		}
	}

	function emit() {
		switch (schemaType) {
			case 'abstract': {
				const labelList = labels.split(',').map((l) => l.trim()).filter(Boolean);
				value = { type: 'abstract', ...(labelList.length > 0 ? { labels: labelList } : {}) };
				break;
			}
			case 'grid': {
				const schema: Record<string, unknown> = { type: 'grid', rows, columns };
				const rl = rowLabels.split(',').map((l) => l.trim()).filter(Boolean);
				const cl = columnLabels.split(',').map((l) => l.trim()).filter(Boolean);
				if (rl.length > 0) schema.row_labels = rl;
				if (cl.length > 0) schema.column_labels = cl;
				value = schema;
				break;
			}
			case 'geo':
				value = { type: 'geo' };
				break;
			default:
				value = null;
				break;
		}
	}

	function onTypeChange(e: Event) {
		schemaType = (e.target as HTMLSelectElement).value as typeof schemaType;
		emit();
	}
</script>

<div class="space-y-3">
	<div>
		<label class="mb-1 block text-sm font-medium text-slate-300" for="schema-type">Location schema</label>
		<select id="schema-type" class="input" value={schemaType} on:change={onTypeChange}>
			<option value="">None</option>
			<option value="abstract">Labels</option>
			<option value="grid">Grid</option>
			<option value="geo">Geographic</option>
		</select>
	</div>

	{#if schemaType === 'abstract'}
		<div>
			<label class="mb-1 block text-xs text-slate-400" for="schema-labels">
				Predefined labels <span class="text-slate-500">(comma-separated, optional)</span>
			</label>
			<input
				id="schema-labels"
				class="input text-sm"
				placeholder="e.g. top shelf, middle shelf, bottom shelf"
				bind:value={labels}
				on:input={emit}
			/>
		</div>

	{:else if schemaType === 'grid'}
		<div class="grid grid-cols-2 gap-3">
			<div>
				<label class="mb-1 block text-xs text-slate-400" for="schema-rows">Rows</label>
				<input id="schema-rows" class="input text-sm" type="number" min="1" max="100" bind:value={rows} on:input={emit} />
			</div>
			<div>
				<label class="mb-1 block text-xs text-slate-400" for="schema-cols">Columns</label>
				<input id="schema-cols" class="input text-sm" type="number" min="1" max="100" bind:value={columns} on:input={emit} />
			</div>
		</div>
		<div>
			<label class="mb-1 block text-xs text-slate-400" for="schema-row-labels">
				Row labels <span class="text-slate-500">(comma-separated, optional)</span>
			</label>
			<input id="schema-row-labels" class="input text-sm" placeholder="e.g. A, B, C" bind:value={rowLabels} on:input={emit} />
		</div>
		<div>
			<label class="mb-1 block text-xs text-slate-400" for="schema-col-labels">
				Column labels <span class="text-slate-500">(comma-separated, optional)</span>
			</label>
			<input id="schema-col-labels" class="input text-sm" placeholder="e.g. 1, 2, 3" bind:value={columnLabels} on:input={emit} />
		</div>
	{/if}
</div>
