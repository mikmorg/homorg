<script lang="ts">
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { api } from '$api/client.js';
	import type { Item, Category, Tag, Condition, UpdateItemRequest, ExternalCode } from '$api/types.js';
	import { CONDITIONS, CONDITION_LABELS } from '$api/types.js';
	import CoordinateInput from '$lib/components/CoordinateInput.svelte';
	import LocationSchemaEditor from '$lib/components/LocationSchemaEditor.svelte';
	import { toast } from '$stores/toast.js';
	import { detectBarcodeType, STANDARD_CODE_TYPES, STANDARD_CODE_TYPE_VALUES } from '$lib/barcode-type.js';
	import type { CameraScanner } from '$lib/scanner/camera-scanner.js';

	let itemId = $derived(page.params.itemId!);
	// H-12: Re-load when itemId changes (client-side navigation between items)
	$effect(() => { if (itemId) loadItem(itemId); });

	let item: Item | null = $state(null);
	let loading: boolean = $state(true);
	let saving: boolean = $state(false);
	let error: string = $state('');
	let saveError: string = $state('');
	let barcodeError: string = $state('');

	// Form state
	let name: string = $state('');
	let description: string = $state('');
	let categoryId: string | null = $state(null);
	let condition: Condition | '' = $state('');
	let selectedTagIds: Set<string> = $state(new Set());
	let acquisitionDate: string = $state('');
	let acquisitionCost: string = $state('');
	let currentValue: string = $state('');
	let currency: string = $state('');
	let warrantyExpiry: string = $state('');
	let coordinateValue: unknown | null = $state(null);
	let weightGrams: string = $state('');
	let isFungible: boolean = $state(false);
	let fungibleUnit: string = $state('');
	let fungibleQuantity: string = $state('');
	let locationSchemaValue: unknown | null = $state(null);
	let schemaLabelRenames: Record<string, string> = $state({});
	let isContainer: boolean = $state(false);
	let systemBarcode: string = $state('');
	let externalCodes: ExternalCode[] = $state([]);
	let generatingBarcode: boolean = $state(false);

	// Per-key metadata editor. `isJson=true` means the value is an object/array
	// edited as raw JSON; scalars get auto-coerced (numbers, booleans, null).
	type MetadataRow = { key: string; value: string; isJson: boolean };
	let metadataEntries: MetadataRow[] = $state([]);
	let metadataErrors: Record<number, string> = $state({});

	// Inline camera scanner for adding external codes
	let scanningForCode: boolean = $state(false);
	let scanContainer: HTMLDivElement | null = $state(null);
	let activeScanCam: CameraScanner | null = null;

	$effect(() => {
		if (!scanningForCode || !scanContainer) return;
		let cancelled = false;
		let cam: CameraScanner | null = null;

		(async () => {
			const { CameraScanner } = await import('$lib/scanner/camera-scanner.js');
			if (cancelled) return;
			cam = new CameraScanner();
			activeScanCam = cam;
			cam.onScan((e) => {
				if (cancelled) return;
				const type = detectBarcodeType(e.barcode, e.format);
				externalCodes = [...externalCodes, { type, value: e.barcode }];
				stopCodeScan();
			});
			try {
				await cam.start();
				if (cancelled) { cam.stop(); return; }
				if (scanContainer) {
					cam.videoElement.className = 'w-full rounded-lg object-cover max-h-40';
					scanContainer.appendChild(cam.videoElement);
				}
			} catch {
				if (!cancelled) barcodeError = 'Camera unavailable — scan with a physical scanner or type the code manually';
				stopCodeScan();
			}
		})();

		return () => {
			cancelled = true;
			cam?.stop();
			activeScanCam = null;
		};
	});

	// Taxonomy data
	let categories: Category[] = $state([]);
	let allTags: Tag[] = $state([]);
	let parentItem: Item | null = $state(null);

	// Image upload
	let uploading: boolean = $state(false);
	let uploadError: string = $state('');
	let imageChanged: boolean = $state(false);

	let loadedItemId: string = $state('');

	async function loadItem(id: string) {
		if (id === loadedItemId) return;
		loadedItemId = id;
		loading = true;
		error = '';
		saveError = '';
		imageChanged = false;
		item = null;
		parentItem = null;
		try {
			const [fetchedItem, cats, tags] = await Promise.all([
				api.items.get(id),
				api.categories.list(),
				api.tags.list()
			]);
			if (id !== loadedItemId) return; // stale guard
			item = fetchedItem;
			categories = cats;
			allTags = tags;

			// Populate form
			name = item.name ?? '';
			description = item.description ?? '';
			categoryId = item.category_id ?? null;
			condition = (item.condition as Condition) ?? '';
			selectedTagIds = new Set(
				allTags.filter((t) => item!.tags.includes(t.name)).map((t) => t.id)
			);
			acquisitionDate = item.acquisition_date ?? '';
			acquisitionCost = item.acquisition_cost ?? '';
			currentValue = item.current_value ?? '';
			currency = item.currency ?? '';
			warrantyExpiry = item.warranty_expiry ?? '';
			coordinateValue = item.coordinate;
			weightGrams = item.weight_grams ?? '';
			isFungible = item.is_fungible;
			fungibleUnit = item.fungible_unit ?? '';
			fungibleQuantity = item.fungible_quantity != null ? String(item.fungible_quantity) : '';
			locationSchemaValue = item.location_schema;
			isContainer = item.is_container;
			systemBarcode = item.system_barcode ?? '';
			externalCodes = item.external_codes ? [...item.external_codes] : [];
			metadataEntries = Object.entries(item.metadata ?? {}).map(([k, v]) => {
				if (v === null || typeof v !== 'object') {
					return { key: k, value: v === null ? 'null' : String(v), isJson: false };
				}
				return { key: k, value: JSON.stringify(v, null, 2), isJson: true };
			});
			metadataErrors = {};
			if (item.parent_id) {
				parentItem = await api.items.get(item.parent_id);
			}
		} catch (err) {
			if (id !== loadedItemId) return;
			error = err instanceof Error ? err.message : 'Failed to load item';
		} finally {
			if (id === loadedItemId) loading = false;
		}
	}

	async function save() {
		if (!item) return;
		saving = true;
		saveError = '';

		const updates: UpdateItemRequest = {};

		if (name !== (item.name ?? '')) updates.name = name;
		if (description !== (item.description ?? '')) updates.description = description;

		const newCategoryId = categoryId || undefined;
		if (newCategoryId !== (item.category_id ?? undefined)) {
			// Find category name to send
			const cat = categories.find((c) => c.id === newCategoryId);
			if (cat) updates.category = cat.name;
			else if (!newCategoryId) updates.category = '';
		}

		const newCondition: Condition | null = (condition as Condition) || null;
		if (newCondition !== (item.condition ?? null)) {
			updates.condition = newCondition;
		}

		const newTagNames = allTags
			.filter((t) => selectedTagIds.has(t.id))
			.map((t) => t.name);
		const oldTagNames = item.tags ?? [];
		if (JSON.stringify(newTagNames.sort()) !== JSON.stringify([...oldTagNames].sort())) {
			updates.tags = newTagNames;
		}

		const newAcqDate: string | null = acquisitionDate || null;
		if (newAcqDate !== (item.acquisition_date ?? null)) {
			updates.acquisition_date = newAcqDate;
		}

		// B5: send as string to preserve decimal precision (backend parses as Decimal).
		const newAcqCost: string | null = acquisitionCost.trim() !== '' && Number.isFinite(parseFloat(acquisitionCost)) ? acquisitionCost.trim() : null;
		if (newAcqCost !== (item.acquisition_cost ?? null)) {
			updates.acquisition_cost = newAcqCost;
		}

		const newCurrVal: string | null = currentValue.trim() !== '' && Number.isFinite(parseFloat(currentValue)) ? currentValue.trim() : null;
		if (newCurrVal !== (item.current_value ?? null)) {
			updates.current_value = newCurrVal;
		}

		const newWarranty: string | null = warrantyExpiry || null;
		if (newWarranty !== (item.warranty_expiry ?? null)) {
			updates.warranty_expiry = newWarranty;
		}

		const newCurrency: string | null = currency || null;
		if (newCurrency !== (item.currency ?? null)) {
			updates.currency = newCurrency;
		}

		const parsedWeight = parseFloat(weightGrams);
		const newWeight: number | null = Number.isFinite(parsedWeight) ? parsedWeight : null;
		const oldWeight: number | null = item.weight_grams ? parseFloat(item.weight_grams) : null;
		if (newWeight !== oldWeight) {
			updates.weight_grams = newWeight;
		}

		if (isFungible !== item.is_fungible) {
			updates.is_fungible = isFungible;
		}

		const newFungibleUnit: string | null = fungibleUnit || null;
		if (newFungibleUnit !== (item.fungible_unit ?? null)) {
			updates.fungible_unit = newFungibleUnit;
		}

		if (JSON.stringify(coordinateValue) !== JSON.stringify(item.coordinate)) {
			updates.coordinate = coordinateValue;
		}

		if (isContainer !== item.is_container) {
			updates.is_container = isContainer;
		}

		const newSystemBarcode: string | null = systemBarcode.trim() || null;
		if (newSystemBarcode !== (item.system_barcode ?? null)) {
			updates.system_barcode = newSystemBarcode;
		}

		const newExternalCodes = externalCodes.filter(c => c.type.trim() && c.value.trim());
		if (JSON.stringify(newExternalCodes) !== JSON.stringify(item.external_codes ?? [])) {
			updates.external_codes = newExternalCodes;
		}

		const schemaChanged = isContainer &&
			JSON.stringify(locationSchemaValue) !== JSON.stringify(item.location_schema);

		const parsedQty = parseInt(fungibleQuantity);
		const newQty = Number.isFinite(parsedQty) ? parsedQty : null;
		const oldQty = item.fungible_quantity ?? null;
		const quantityChanged = isFungible && newQty !== null && newQty !== oldQty;

		// Build metadata object from per-key rows. Reject on invalid JSON;
		// otherwise diff against current and include in updates if changed.
		const newMetadata: Record<string, unknown> = {};
		const newMetaErrors: Record<number, string> = {};
		let metadataValid = true;
		metadataEntries.forEach((entry, i) => {
			const k = entry.key.trim();
			if (!k) return;
			if (entry.isJson) {
				try {
					newMetadata[k] = JSON.parse(entry.value);
				} catch (err) {
					newMetaErrors[i] = err instanceof Error ? err.message : 'Invalid JSON';
					metadataValid = false;
				}
			} else {
				newMetadata[k] = coerceMetadataValue(entry.value);
			}
		});
		metadataErrors = newMetaErrors;
		if (!metadataValid) {
			saveError = 'Fix invalid JSON in metadata before saving.';
			saving = false;
			return;
		}
		if (JSON.stringify(newMetadata) !== JSON.stringify(item.metadata ?? {})) {
			updates.metadata = newMetadata;
		}

		if (Object.keys(updates).length === 0 && !schemaChanged && !quantityChanged && !imageChanged) {
			saveError = 'No changes to save.';
			saving = false;
			return;
		}

		try {
			if (Object.keys(updates).length > 0) {
				await api.items.update(itemId, updates);
				// Refresh item so a retry only re-applies the schema, not already-saved fields.
				item = await api.items.get(itemId);
			}
			if (schemaChanged && isContainer) {
				await api.containers.updateSchema(itemId, locationSchemaValue, schemaLabelRenames);
			}
			if (quantityChanged && newQty !== null) {
				await api.items.adjustQuantity(itemId, { new_quantity: newQty });
			}
			toast('Changes saved', 'success');
			goto(`/browse/item/${itemId}`);
		} catch (err) {
			saveError = err instanceof Error ? err.message : 'Save failed';
		} finally {
			saving = false;
		}
	}

	function toggleTag(tagId: string) {
		if (selectedTagIds.has(tagId)) {
			selectedTagIds.delete(tagId);
		} else {
			selectedTagIds.add(tagId);
		}
		selectedTagIds = new Set(selectedTagIds);
	}

	async function handleImageUpload(e: Event) {
		const input = e.target as HTMLInputElement;
		const file = input.files?.[0];
		if (!file) return;

		uploading = true;
		uploadError = '';
		try {
			await api.items.uploadImage(itemId, file);
			item = await api.items.get(itemId);
			imageChanged = true;
			toast('Image added', 'success');
		} catch (err) {
			uploadError = err instanceof Error ? err.message : 'Upload failed';
		} finally {
			uploading = false;
			input.value = '';
		}
	}

	async function removeImage(idx: number) {
		if (!confirm('Remove this image?')) return;
		try {
			await api.items.removeImage(itemId, idx);
			item = await api.items.get(itemId);
			imageChanged = true;
		} catch (err) {
			uploadError = err instanceof Error ? err.message : 'Remove failed';
		}
	}

	async function generateBarcode() {
		generatingBarcode = true;
		try {
			const result = await api.barcodes.generate();
			systemBarcode = result.barcode;
		} catch (err) {
			saveError = err instanceof Error ? err.message : 'Failed to generate barcode';
		} finally {
			generatingBarcode = false;
		}
	}

	function addExternalCode() {
		externalCodes = [...externalCodes, { type: '', value: '' }];
	}

	function coerceMetadataValue(s: string): unknown {
		const t = s.trim();
		if (t === '') return '';
		if (t === 'null') return null;
		if (t === 'true') return true;
		if (t === 'false') return false;
		if (/^-?\d+(\.\d+)?$/.test(t)) return Number(t);
		return s;
	}

	function addMetadataField() {
		metadataEntries = [...metadataEntries, { key: '', value: '', isJson: false }];
	}

	function removeMetadataField(idx: number) {
		metadataEntries = metadataEntries.filter((_, i) => i !== idx);
		// Reindex errors after removal.
		const reindexed: Record<number, string> = {};
		Object.entries(metadataErrors).forEach(([k, v]) => {
			const n = Number(k);
			if (n < idx) reindexed[n] = v;
			else if (n > idx) reindexed[n - 1] = v;
		});
		metadataErrors = reindexed;
	}

	function toggleMetadataJson(idx: number) {
		const row = metadataEntries[idx];
		if (!row) return;
		if (row.isJson) {
			// JSON → scalar: keep raw text so the user can see what they had.
			metadataEntries[idx] = { ...row, isJson: false };
		} else {
			// Scalar → JSON: try to pretty-print the coerced value.
			try {
				const coerced = coerceMetadataValue(row.value);
				metadataEntries[idx] = { ...row, value: JSON.stringify(coerced, null, 2), isJson: true };
			} catch {
				metadataEntries[idx] = { ...row, isJson: true };
			}
		}
		metadataEntries = [...metadataEntries];
		const { [idx]: _removed, ...rest } = metadataErrors;
		metadataErrors = rest;
	}

	function removeExternalCode(idx: number) {
		if (!confirm('Remove this external code?')) return;
		externalCodes = externalCodes.filter((_, i) => i !== idx);
	}

	function stopCodeScan() {
		activeScanCam?.stop();
		activeScanCam = null;
		scanningForCode = false;
	}

	function autoDetectType(code: ExternalCode) {
		if (!code.type.trim() && code.value.trim()) {
			code.type = detectBarcodeType(code.value);
			externalCodes = [...externalCodes];
		}
	}

	function setCodeType(code: ExternalCode, value: string) {
		if (value === '__custom__') {
			// Switching to custom — clear only if it was a standard type so the
			// text input appears ready to accept a new value.
			if (STANDARD_CODE_TYPE_VALUES.has(code.type)) code.type = '';
		} else {
			code.type = value;
		}
		externalCodes = [...externalCodes];
	}
</script>

<svelte:head>
	<title>Edit {item?.name ?? 'Item'} — Homorg</title>
</svelte:head>

<div class="flex h-full flex-col">
	<header class="flex items-center gap-2 border-b border-slate-800 px-3 py-2">
		<button class="btn btn-icon text-slate-400" onclick={() => goto(`/browse/item/${itemId}`)} aria-label="Back">
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M15 18l-6-6 6-6" />
			</svg>
		</button>
		<h1 class="flex-1 text-base font-semibold text-slate-100 truncate">
			Edit {item?.name ?? 'Item'}
		</h1>
		<button class="btn btn-primary text-xs" onclick={save} disabled={saving || loading}>
			{saving ? 'Saving…' : 'Save'}
		</button>
	</header>

	<div class="flex-1 overflow-y-auto p-4">
		{#if loading}
			<div class="flex h-32 items-center justify-center">
				<div class="h-6 w-6 animate-spin rounded-full border-2 border-slate-600 border-t-indigo-500"></div>
			</div>
		{:else if error}
			<div class="rounded-lg bg-red-950 px-4 py-3 text-sm text-red-300 border border-red-800">{error}</div>
		{:else if item}
			<div class="space-y-4">
				{#if saveError}
					<div class="rounded-lg bg-red-950 px-4 py-3 text-sm text-red-300 border border-red-800">{saveError}</div>
				{/if}

				<!-- Name -->
				<div>
					<label class="mb-1 block text-sm font-medium text-slate-300" for="edit-name">Name</label>
					<input id="edit-name" class="input" bind:value={name} placeholder="Item name" />
				</div>

				<!-- Description -->
				<div>
					<label class="mb-1 block text-sm font-medium text-slate-300" for="edit-desc">Description</label>
					<textarea id="edit-desc" class="input min-h-20 resize-y" bind:value={description} placeholder="Optional description" rows="3"></textarea>
				</div>

				<!-- Category -->
				<div>
					<label class="mb-1 block text-sm font-medium text-slate-300" for="edit-category">Category</label>
					<select id="edit-category" class="input" bind:value={categoryId}>
						<option value={null}>None</option>
						{#each categories as cat (cat.id)}
							<option value={cat.id}>{cat.name}</option>
						{/each}
					</select>
				</div>

				<!-- Condition -->
				<div>
					<label class="mb-1 block text-sm font-medium text-slate-300" for="edit-condition">Condition</label>
					<select id="edit-condition" class="input" bind:value={condition}>
						<option value="">Not set</option>
						{#each CONDITIONS as c}
							<option value={c}>{CONDITION_LABELS[c] ?? c}</option>
						{/each}
					</select>
				</div>

				<!-- Tags -->
				{#if allTags.length > 0}
					<div>
						<p class="mb-2 text-sm font-medium text-slate-300">Tags</p>
						<div class="flex flex-wrap gap-1.5">
							{#each allTags as tag (tag.id)}
								<button
									type="button"
									class="rounded-full px-3 py-1 text-xs font-medium transition-colors
										{selectedTagIds.has(tag.id) ? 'bg-indigo-600 text-white' : 'bg-slate-700 text-slate-300 hover:bg-slate-600'}"
									onclick={() => toggleTag(tag.id)}
								>
									{tag.name}
								</button>
							{/each}
						</div>
					</div>
				{/if}

				<!-- Container toggle -->
				<div class="card p-3">
					<label class="flex items-center justify-between cursor-pointer" for="edit-is-container">
						<div>
							<p class="text-sm font-medium text-slate-300">Container</p>
							<p class="text-xs text-slate-500">Allow this item to hold other items</p>
						</div>
						<input
							id="edit-is-container"
							type="checkbox"
							class="h-5 w-5 rounded border-slate-600 bg-slate-800 text-indigo-500 focus:ring-indigo-500 focus:ring-offset-0"
							bind:checked={isContainer}
							disabled={item.is_fungible}
						/>
					</label>
					{#if item.is_fungible}
						<p class="mt-1 text-xs text-amber-400">Fungible items cannot be containers</p>
					{/if}
					{#if !isContainer && item.is_container}
						<p class="mt-1 text-xs text-amber-400">Container must be empty to convert</p>
					{/if}
				</div>

				<!-- Fungible toggle -->
				<div class="card p-3">
					<label class="flex items-center justify-between cursor-pointer" for="edit-is-fungible">
						<div>
							<p class="text-sm font-medium text-slate-300">Fungible</p>
							<p class="text-xs text-slate-500">Track quantity instead of uniqueness</p>
						</div>
						<input
							id="edit-is-fungible"
							type="checkbox"
							class="h-5 w-5 rounded border-slate-600 bg-slate-800 text-indigo-500 focus:ring-indigo-500 focus:ring-offset-0"
							bind:checked={isFungible}
							disabled={isContainer}
						/>
					</label>
					{#if isContainer}
						<p class="mt-1 text-xs text-amber-400">Containers cannot be fungible</p>
					{/if}
					{#if isFungible}
						<div class="mt-2 grid grid-cols-2 gap-3">
							<div>
								<label class="mb-1 block text-xs text-slate-400" for="edit-fungible-qty">Quantity</label>
								<input id="edit-fungible-qty" class="input text-sm" type="number" step="1" min="0" bind:value={fungibleQuantity} placeholder="0" />
							</div>
							<div>
								<label class="mb-1 block text-xs text-slate-400" for="edit-fungible-unit">Unit</label>
								<input id="edit-fungible-unit" class="input text-sm" bind:value={fungibleUnit} placeholder="e.g. pieces, ml, kg" />
							</div>
						</div>
					{/if}
				</div>

				<!-- Coordinate (M-18: {#key} forces remount on navigation) -->
				{#if parentItem?.location_schema || item.coordinate}
					<div class="card p-3">
						{#key itemId}
							<CoordinateInput schema={parentItem?.location_schema} bind:value={coordinateValue} />
						{/key}
					</div>
				{/if}

				<!-- Location Schema (containers only) -->
				{#if isContainer}
					<div class="card p-3">
						{#key itemId}
							<LocationSchemaEditor bind:value={locationSchemaValue} bind:labelRenames={schemaLabelRenames} />
						{/key}
					</div>
				{/if}

				<!-- Barcodes -->
				<div class="card p-3 space-y-3">
					<p class="text-xs text-slate-400 uppercase tracking-wide">Barcodes</p>

					<!-- System barcode -->
					<div>
						<label class="mb-1 block text-xs text-slate-400" for="edit-sys-barcode">System barcode</label>
						<div class="flex gap-2">
							<input
								id="edit-sys-barcode"
								class="input flex-1 font-mono text-sm"
								bind:value={systemBarcode}
								placeholder="None assigned"
							/>
							{#if !systemBarcode.trim()}
								<button
									type="button"
									class="btn btn-secondary text-xs flex-shrink-0"
									onclick={generateBarcode}
									disabled={generatingBarcode}
								>
									{generatingBarcode ? '…' : 'Generate'}
								</button>
							{/if}
						</div>
					</div>

					<!-- External codes -->
					<div>
						<div class="flex items-center justify-between mb-1">
							<span class="text-xs text-slate-400">External codes (UPC, ISBN, EAN…)</span>
							<div class="flex gap-2">
								{#if !scanningForCode}
									<button type="button" class="text-xs text-indigo-400 hover:text-indigo-300" onclick={() => { barcodeError = ''; scanningForCode = true; }}>Scan</button>
									<button type="button" class="text-xs text-indigo-400 hover:text-indigo-300" onclick={addExternalCode}>+ Add</button>
								{:else}
									<button type="button" class="text-xs text-red-400 hover:text-red-300" onclick={stopCodeScan}>Cancel</button>
								{/if}
							</div>
						</div>

						{#if scanningForCode}
							<div class="rounded-lg overflow-hidden bg-slate-900 border border-slate-700">
								<div bind:this={scanContainer} class="w-full"></div>
								<p class="text-center text-xs text-slate-500 py-1">Point camera at barcode — type is detected automatically</p>
							</div>
						{/if}

						{#if barcodeError}
							<p class="mt-1 text-xs text-red-400">{barcodeError}</p>
						{/if}

						{#if externalCodes.length === 0 && !scanningForCode}
							<p class="text-xs text-slate-600 italic">No external codes</p>
						{:else if externalCodes.length > 0}
							<div class="space-y-2 mt-2">
								{#each externalCodes as code, idx}
									<div class="space-y-1">
										<div class="flex gap-2 items-center">
											<select
												class="input w-28 text-xs flex-shrink-0"
												value={STANDARD_CODE_TYPE_VALUES.has(code.type) ? code.type : (code.type ? '__custom__' : '')}
												onchange={(e) => setCodeType(code, (e.currentTarget as HTMLSelectElement).value)}
												aria-label="Code type"
											>
												<option value="">Type…</option>
												{#each STANDARD_CODE_TYPES as t}
													<option value={t.value} title={t.description}>{t.value}</option>
												{/each}
												<option disabled>──────</option>
												<option value="__custom__">Custom…</option>
											</select>
											<input
												class="input flex-1 font-mono text-xs"
												bind:value={code.value}
												placeholder="Value"
												aria-label="Code value"
												onblur={() => autoDetectType(code)}
											/>
											<button
												type="button"
												class="text-red-400 hover:text-red-300 flex-shrink-0 px-1"
												onclick={() => removeExternalCode(idx)}
												aria-label="Remove code"
											>
												&times;
											</button>
										</div>
										{#if code.type && !STANDARD_CODE_TYPE_VALUES.has(code.type)}
											<input
												class="input w-full text-xs font-mono"
												bind:value={code.type}
												placeholder="Custom type name"
												aria-label="Custom code type"
											/>
										{/if}
									</div>
								{/each}
							</div>
						{/if}
					</div>
				</div>

				<!-- Images -->
				<div>
					<p class="mb-2 text-sm font-medium text-slate-300">Images</p>
					{#if item.images && item.images.length > 0}
						<div class="mb-3 grid grid-cols-3 gap-2">
							{#each item.images as img, idx}
								<div class="relative group">
									<img src={img.path} alt={img.caption ?? 'Image'} class="w-full h-24 rounded-lg object-cover" />
									<button
										class="absolute top-1 right-1 flex h-6 w-6 items-center justify-center rounded-full bg-red-600 text-white text-xs shadow"
										onclick={() => removeImage(idx)}
									>
										&times;
									</button>
								</div>
							{/each}
						</div>
					{/if}
					<label class="btn btn-secondary w-full cursor-pointer text-sm">
						{uploading ? 'Uploading…' : 'Add image'}
						<input type="file" accept="image/*" class="hidden" onchange={handleImageUpload} disabled={uploading} />
					</label>
					{#if uploadError}
						<p class="mt-1 text-xs text-red-400">{uploadError}</p>
					{/if}
				</div>

				<!-- Valuation -->
				<div class="card p-3 space-y-3">
					<p class="text-xs text-slate-400 uppercase tracking-wide">Valuation</p>
					<div class="grid grid-cols-2 gap-3">
						<div>
							<label class="mb-1 block text-xs text-slate-400" for="edit-acq-date">Acquisition date</label>
							<input id="edit-acq-date" class="input text-sm" type="date" bind:value={acquisitionDate} />
						</div>
						<div>
							<label class="mb-1 block text-xs text-slate-400" for="edit-acq-cost">Acquisition cost</label>
							<input id="edit-acq-cost" class="input text-sm" type="number" step="0.01" min="0" bind:value={acquisitionCost} placeholder="0.00" />
						</div>
						<div>
							<label class="mb-1 block text-xs text-slate-400" for="edit-curr-val">Current value</label>
							<input id="edit-curr-val" class="input text-sm" type="number" step="0.01" min="0" bind:value={currentValue} placeholder="0.00" />
						</div>
						<div>
							<label class="mb-1 block text-xs text-slate-400" for="edit-warranty">Warranty expiry</label>
							<input id="edit-warranty" class="input text-sm" type="date" bind:value={warrantyExpiry} />
						</div>
					</div>
				</div>

				<!-- Metadata (per-key editor) -->
				<div class="card p-3 space-y-3">
					<div class="flex items-center justify-between">
						<p class="text-xs text-slate-400 uppercase tracking-wide">Metadata</p>
						<button type="button" class="text-xs text-indigo-400 hover:text-indigo-300" onclick={addMetadataField}>
							+ Add field
						</button>
					</div>
					{#if metadataEntries.length === 0}
						<p class="text-xs text-slate-500">No metadata. Click “Add field” to store extra structured info (e.g. <code class="text-slate-300">model_number</code>, <code class="text-slate-300">serial</code>, <code class="text-slate-300">color</code>).</p>
					{:else}
						<div class="space-y-2">
							{#each metadataEntries as entry, i (i)}
								<div class="space-y-1">
									<div class="flex gap-2 items-start">
										<input
											class="input text-sm flex-1 font-mono"
											type="text"
											placeholder="key"
											bind:value={entry.key}
										/>
										{#if entry.isJson}
											<textarea
												class="input text-sm flex-[2] font-mono"
												rows="3"
												placeholder={'{\n  "key": "value"\n}'}
												bind:value={entry.value}
											></textarea>
										{:else}
											<input
												class="input text-sm flex-[2]"
												type="text"
												placeholder="value"
												bind:value={entry.value}
											/>
										{/if}
										<button
											type="button"
											class="btn btn-icon text-slate-400 hover:text-indigo-300 flex-shrink-0"
											title={entry.isJson ? 'Switch to scalar value' : 'Edit as raw JSON'}
											onclick={() => toggleMetadataJson(i)}
										>
											<span class="text-xs font-mono px-1">{entry.isJson ? 'abc' : '{}'}</span>
										</button>
										<button
											type="button"
											class="btn btn-icon text-slate-400 hover:text-red-400 flex-shrink-0"
											aria-label="Remove field"
											onclick={() => removeMetadataField(i)}
										>
											<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
												<path d="M3 6h18M8 6V4a2 2 0 012-2h4a2 2 0 012 2v2m-9 0h14l-1 14a2 2 0 01-2 2H8a2 2 0 01-2-2L5 6h0z" />
											</svg>
										</button>
									</div>
									{#if metadataErrors[i]}
										<p class="text-xs text-red-400">{metadataErrors[i]}</p>
									{/if}
								</div>
							{/each}
						</div>
					{/if}
					<p class="text-xs text-slate-500">
						Scalar values are auto-typed: <code class="text-slate-300">42</code>, <code class="text-slate-300">true</code>, <code class="text-slate-300">null</code> become numbers/booleans/null; anything else is a string. Toggle <code class="text-slate-300">{`{}`}</code> for nested objects or arrays (raw JSON).
					</p>
				</div>

				<!-- Save button (bottom) -->
				<button class="btn btn-primary w-full" onclick={save} disabled={saving}>
					{saving ? 'Saving…' : 'Save changes'}
				</button>
			</div>
		{/if}
	</div>
</div>
