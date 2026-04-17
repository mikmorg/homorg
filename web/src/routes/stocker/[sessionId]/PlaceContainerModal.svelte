<script lang="ts">
	import type { Item, ItemSummary } from '$api/types.js';

	interface Props {
		show: boolean;
		barcode: string;
		typeName: string | null;
		parentQuery: string;
		parentResults: ItemSummary[];
		parentLoading: boolean;
		parentSelected: ItemSummary | Item | null;
		loading: boolean;
		error: string;
		suggestedParent: Item | null;
		onClose: () => void;
		onParentQueryChange: (query: string) => void;
		onParentSelect: (item: ItemSummary | Item) => void;
		onParentClear: () => void;
		onSuggestedParentSelect: () => void;
		onSubmit: () => void;
	}

	const {
		show = false,
		barcode = '',
		typeName = null,
		parentQuery = '',
		parentResults = [],
		parentLoading = false,
		parentSelected = null,
		loading = false,
		error = '',
		suggestedParent = null,
		onClose,
		onParentQueryChange,
		onParentSelect,
		onParentClear,
		onSuggestedParentSelect,
		onSubmit
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
		<div class="rounded-t-2xl bg-slate-900 p-4 pb-8" role="dialog" aria-modal="true" aria-labelledby="place-container-title">
			<div class="mb-4 flex items-center justify-between">
				<div>
					<h2 id="place-container-title" class="text-base font-semibold text-slate-100">Place new container</h2>
					<p class="text-xs text-slate-400 font-mono">
						{barcode}{#if typeName} · {typeName}{/if}
					</p>
				</div>
				<button class="btn btn-icon text-slate-400" onclick={onClose} aria-label="Close">
					<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
						<path d="M18 6L6 18M6 6l12 12" />
					</svg>
				</button>
			</div>

			{#if error}
				<div class="mb-3 rounded-lg bg-red-950 px-3 py-2 text-sm text-red-300 border border-red-800">
					{error}
				</div>
			{/if}

			<div class="space-y-3">
				<div>
					<label class="mb-1 block text-sm font-medium text-slate-300" for="place-parent-search">Parent container</label>
					{#if suggestedParent && !parentSelected}
						<button
							class="mb-1 w-full rounded-lg bg-emerald-500/10 border border-emerald-500/30 px-3 py-2 text-left hover:bg-emerald-500/20 transition-colors"
							onclick={onSuggestedParentSelect}
						>
							<p class="text-xs text-emerald-400 font-medium mb-0.5">Quick option:</p>
							<p class="text-sm text-slate-100">{suggestedParent.name ?? 'Unnamed'} (parent of current)</p>
						</button>
					{/if}
					{#if parentSelected}
						<div class="flex items-center gap-2 rounded-lg bg-indigo-500/10 border border-indigo-500/30 px-3 py-2">
							<span class="flex-1 text-sm text-slate-100">{parentSelected.name ?? 'Unnamed'}</span>
							<button class="text-xs text-slate-400 hover:text-slate-200" onclick={onParentClear}>✕</button>
						</div>
					{:else}
						<input
							id="place-parent-search"
							class="input"
							placeholder="Search containers…"
							value={parentQuery}
							onchange={(e) => onParentQueryChange((e.target as HTMLInputElement).value)}
							oninput={(e) => onParentQueryChange((e.target as HTMLInputElement).value)}
							disabled={loading}
						/>
						{#if parentLoading}
							<div class="mt-1 flex h-8 items-center justify-center">
								<div class="h-4 w-4 animate-spin rounded-full border-2 border-slate-600 border-t-indigo-500"></div>
							</div>
						{:else if parentResults.length > 0}
							<div class="mt-1 max-h-40 overflow-y-auto rounded-lg border border-slate-700 bg-slate-800">
								{#each parentResults as item (item.id)}
									<button
										class="flex w-full items-center gap-2 px-3 py-2 text-left text-sm hover:bg-slate-700"
										onclick={() => onParentSelect(item)}
									>
										<div class="min-w-0 flex-1">
											<p class="truncate text-slate-100">{item.name ?? 'Unnamed'}</p>
											{#if item.parent_name}
												<p class="truncate text-xs text-slate-500">in {item.parent_name}</p>
											{/if}
										</div>
										{#if item.system_barcode}
											<span class="flex-shrink-0 font-mono text-xs text-slate-500">{item.system_barcode}</span>
										{/if}
									</button>
								{/each}
							</div>
						{/if}
					{/if}
				</div>

				<p class="text-xs text-slate-500">Coordinate can be set later in Browse → Edit.</p>

				<button
					class="btn btn-primary w-full"
					onclick={onSubmit}
					disabled={loading || !parentSelected}
				>
					{#if loading}
						<span class="h-4 w-4 animate-spin rounded-full border-2 border-white/30 border-t-white"></span>
					{:else}
						Create container & set as context
					{/if}
				</button>
			</div>
		</div>
	</div>
{/if}
