<script lang="ts">
	import { parseLocationSchema, parseCoordinate } from '$lib/coordinate-helpers.js';
	import { onMount } from 'svelte';

	let { schema = null, value = $bindable(null) }: { schema?: unknown | null; value?: unknown | null } = $props();

	let parsed = $derived(parseLocationSchema(schema));

	// Internal form state, synced from value
	let abstractText = $state('');
	let gridRow = $state(0);
	let gridCol = $state(0);
	let geoLat = $state('');
	let geoLng = $state('');
	let geoLoading = $state(false);
	let geoError = $state('');

	// Initialize from existing value once on mount.
	// Using onMount (not a reactive $effect) prevents the feedback loop where
	// setAbstract writes value → reactive fires → abstractText gets trimmed mid-keystroke.
	onMount(() => {
		const coord = parseCoordinate(value);
		if (coord?.type === 'abstract') abstractText = coord.value;
		else if (coord?.type === 'grid') { gridRow = coord.row; gridCol = coord.column; }
		else if (coord?.type === 'geo') { geoLat = String(coord.latitude); geoLng = String(coord.longitude); }
	});

	function setAbstract(text: string) {
		abstractText = text;
		value = text.trim() ? { type: 'abstract', value: text.trim() } : null;
	}

	function setGrid(row: number, col: number) {
		gridRow = row;
		gridCol = col;
		value = { type: 'grid', row, column: col };
	}

	function setGeo() {
		const lat = parseFloat(geoLat);
		const lng = parseFloat(geoLng);
		if (!isNaN(lat) && !isNaN(lng)) {
			value = { type: 'geo', latitude: lat, longitude: lng };
		} else {
			value = null;
		}
	}

	function useCurrentLocation() {
		if (!navigator.geolocation) { geoError = 'Geolocation not supported'; return; }
		geoLoading = true;
		geoError = '';
		navigator.geolocation.getCurrentPosition(
			(pos) => {
				geoLat = String(pos.coords.latitude);
				geoLng = String(pos.coords.longitude);
				setGeo();
				geoLoading = false;
			},
			(err) => {
				geoError = err.message;
				geoLoading = false;
			},
			{ enableHighAccuracy: true, timeout: 10000 }
		);
	}
</script>

<div class="space-y-2">
	<p class="text-xs text-slate-400 uppercase tracking-wide">Position</p>

	{#if parsed?.type === 'abstract' && parsed.labels && parsed.labels.length > 0}
		<!-- Abstract with predefined labels -->
		{#if abstractText && !parsed.labels.includes(abstractText)}
			<p class="text-xs text-amber-400 mb-1">Position "{abstractText}" no longer exists in this container's schema — select a new position or leave blank.</p>
		{/if}
		<select class="input" value={abstractText} onchange={(e) => setAbstract((e.currentTarget as HTMLSelectElement).value)}>
			<option value="">None</option>
			{#each parsed.labels as lbl}
				<option value={lbl}>{lbl}</option>
			{/each}
		</select>

	{:else if parsed?.type === 'grid'}
		<!-- Grid row/column selectors -->
		{#if gridRow >= parsed.rows || gridCol >= parsed.columns}
			<p class="text-xs text-amber-400 mb-1">Stored position (row {gridRow + 1}, col {gridCol + 1}) is outside the current grid ({parsed.rows}×{parsed.columns}) — select a new position.</p>
		{/if}
		<div class="grid grid-cols-2 gap-2">
			<div>
				<label class="mb-1 block text-xs text-slate-400" for="coord-row">Row</label>
				<select id="coord-row" class="input" value={gridRow} onchange={(e) => setGrid(parseInt((e.currentTarget as HTMLSelectElement).value), gridCol)}>
					{#each Array(parsed.rows) as _, i}
						<option value={i}>{parsed.row_labels?.[i] ?? i + 1}</option>
					{/each}
				</select>
			</div>
			<div>
				<label class="mb-1 block text-xs text-slate-400" for="coord-col">Column</label>
				<select id="coord-col" class="input" value={gridCol} onchange={(e) => setGrid(gridRow, parseInt((e.currentTarget as HTMLSelectElement).value))}>
					{#each Array(parsed.columns) as _, i}
						<option value={i}>{parsed.column_labels?.[i] ?? i + 1}</option>
					{/each}
				</select>
			</div>
		</div>

	{:else if parsed?.type === 'geo'}
		<!-- Geographic lat/lng -->
		<div class="grid grid-cols-2 gap-2">
			<div>
				<label class="mb-1 block text-xs text-slate-400" for="coord-lat">Latitude</label>
				<input id="coord-lat" class="input text-sm" type="number" step="any" min="-90" max="90" bind:value={geoLat} onblur={setGeo} placeholder="0.000000" />
			</div>
			<div>
				<label class="mb-1 block text-xs text-slate-400" for="coord-lng">Longitude</label>
				<input id="coord-lng" class="input text-sm" type="number" step="any" min="-180" max="180" bind:value={geoLng} onblur={setGeo} placeholder="0.000000" />
			</div>
		</div>
		<button class="btn btn-secondary text-xs w-full" type="button" onclick={useCurrentLocation} disabled={geoLoading}>
			{geoLoading ? 'Getting location…' : 'Use current location'}
		</button>
		{#if geoError}
			<p class="text-xs text-red-400">{geoError}</p>
		{/if}

	{:else}
		<!-- Fallback: simple text input for abstract label -->
		<input
			class="input"
			placeholder="e.g. top shelf, drawer 3"
			value={abstractText}
			oninput={(e) => setAbstract((e.currentTarget as HTMLInputElement).value)}
		/>
	{/if}
</div>
