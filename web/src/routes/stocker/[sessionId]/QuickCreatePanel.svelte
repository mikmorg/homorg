<script lang="ts">
	import type { ExternalCode } from '$api/types.js';
	import { STANDARD_CODE_TYPES, STANDARD_CODE_TYPE_VALUES } from '$lib/barcode-type.js';

	interface Props {
		show: boolean;
		name: string;
		quantity: number;
		barcode: string;
		externalCode: ExternalCode | null;
		loading: boolean;
		error: string;
		containerName: string | null;
		containerSet: boolean;
		onClose: () => void;
		onSubmit: (e: SubmitEvent) => void;
		onNameChange: (name: string) => void;
		onQuantityChange: (qty: number) => void;
		onBarcodeChange: (code: string) => void;
		onExternalCodeChange: (code: ExternalCode | null) => void;
		onExternalCodeTypeChange: (type: string) => void;
	}

	const {
		show = false,
		name = '',
		quantity = 1,
		barcode = '',
		externalCode = null,
		loading = false,
		error = '',
		containerName = null,
		containerSet = false,
		onClose,
		onSubmit,
		onNameChange,
		onQuantityChange,
		onBarcodeChange,
		onExternalCodeChange,
		onExternalCodeTypeChange
	}: Props = $props();
</script>

{#if show}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="fixed inset-0 z-50 flex flex-col justify-end bg-black/60"
		onclick={(e) => {
			if (e.target === e.currentTarget) onClose();
		}}
		onkeydown={(e) => e.key === 'Escape' && onClose()}
	>
		<div class="rounded-t-2xl bg-slate-900 p-4 pb-8" role="dialog" aria-modal="true" aria-labelledby="quick-create-title">
			<div class="mb-4 flex items-center justify-between">
				<h2 id="quick-create-title" class="text-base font-semibold text-slate-100">Quick create item</h2>
				<button class="btn btn-icon text-slate-400" onclick={onClose} aria-label="Close">
					<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
						<path d="M18 6L6 18M6 6l12 12" />
					</svg>
				</button>
			</div>

			{#if !containerSet}
				<div class="mb-3 rounded-lg bg-amber-950 px-3 py-2 text-sm text-amber-300 border border-amber-800">
					No container context set. Scan a container first.
				</div>
			{/if}

			{#if error}
				<div class="mb-3 rounded-lg bg-red-950 px-3 py-2 text-sm text-red-300 border border-red-800">
					{error}
				</div>
			{/if}

			<form class="space-y-3" onsubmit={onSubmit}>
				<div>
					<label class="mb-1 block text-sm font-medium text-slate-300" for="qc-name">Name *</label>
					<input
						id="qc-name"
						class="input"
						placeholder="e.g. 9V Battery"
						value={name}
						onchange={(e) => onNameChange((e.target as HTMLInputElement).value)}
						required
						disabled={loading}
					/>
				</div>

				<div class="flex gap-3">
					<div class="flex-1">
						<label class="mb-1 block text-sm font-medium text-slate-300" for="qc-qty">Quantity</label>
						<input
							id="qc-qty"
							class="input"
							type="number"
							min="1"
							value={quantity}
							onchange={(e) => onQuantityChange(Number((e.target as HTMLInputElement).value))}
							disabled={loading}
						/>
					</div>
					<div class="flex-1">
						<label class="mb-1 block text-sm font-medium text-slate-300" for="qc-barcode">
							{externalCode ? 'External code' : 'Barcode'}
						</label>
						{#if externalCode}
							<div class="flex gap-1.5">
								<select
									class="input text-xs w-24 flex-shrink-0"
									value={externalCode.type}
									onchange={(e) => onExternalCodeTypeChange((e.target as HTMLSelectElement).value)}
									disabled={loading}
									aria-label="Code type"
								>
									{#each STANDARD_CODE_TYPES as t}
										<option value={t.value} title={t.description}>{t.value}</option>
									{/each}
									{#if !STANDARD_CODE_TYPE_VALUES.has(externalCode.type)}
										<option value={externalCode.type}>{externalCode.type}</option>
									{/if}
								</select>
								<input
									id="qc-barcode"
									class="input flex-1 font-mono text-xs min-w-0"
									value={externalCode.value}
									onchange={(e) => {
										if (externalCode) {
											onExternalCodeChange({ ...externalCode, value: (e.target as HTMLInputElement).value });
										}
									}}
									disabled={loading}
								/>
							</div>
						{:else}
							<input
								id="qc-barcode"
								class="input font-mono text-xs"
								placeholder="scanned"
								value={barcode}
								onchange={(e) => onBarcodeChange((e.target as HTMLInputElement).value)}
								disabled={loading}
							/>
						{/if}
					</div>
				</div>

				<div class="pt-1 text-xs text-slate-400">
					→ Will be placed in: <span class="font-medium text-slate-200">{containerName ?? 'none'}</span>
				</div>

				<button type="submit" class="btn btn-primary w-full" disabled={loading || !containerSet}>
					{#if loading}
						<span class="h-4 w-4 animate-spin rounded-full border-2 border-white/30 border-t-white"></span>
					{:else}
						Create & place
					{/if}
				</button>
			</form>
		</div>
	</div>
{/if}
