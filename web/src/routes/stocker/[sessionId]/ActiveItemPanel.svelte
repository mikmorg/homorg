<script lang="ts">
	import type { Item } from '$api/types.js';

	interface Props {
		show: boolean;
		item: Item | null;
		loading: boolean;
		error: string;
		lightboxUrl: string | null;
		onClose: () => void;
		onLightboxOpen: (url: string) => void;
	}

	const { show = false, item = null, loading = false, error = '', lightboxUrl, onClose, onLightboxOpen }: Props = $props();
</script>

{#if show}
	<div
		class="fixed inset-0 z-50 flex flex-col justify-end bg-black/60"
		onclick={(e) => {
			if (e.target === e.currentTarget) onClose();
		}}
		onkeydown={(e) => e.key === 'Escape' && onClose()}
		role="dialog"
		aria-modal="true"
		aria-labelledby="item-panel-title"
		tabindex="-1"
	>
		<div class="max-h-[80vh] overflow-y-auto rounded-t-2xl bg-slate-900 p-4 pb-8">
			<div class="mb-3 flex items-center justify-between">
				<h2 id="item-panel-title" class="text-base font-semibold text-slate-100 truncate">
					{item?.name || (loading ? 'Loading…' : 'Item')}
				</h2>
				<button class="btn btn-icon text-slate-400" onclick={onClose} aria-label="Close">
					<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
						><path d="M18 6L6 18M6 6l12 12" /></svg
					>
				</button>
			</div>

			{#if loading}
				<div class="flex h-24 items-center justify-center"
					><div class="h-6 w-6 animate-spin rounded-full border-2 border-slate-600 border-t-emerald-500"></div></div
				>
			{:else if error}
				<p class="text-sm text-red-400">{error}</p>
			{:else if item}
				{#if item.images.length > 0}
					<div class="mb-3 flex gap-2 overflow-x-auto pb-1">
						{#each item.images as img}
							<button class="flex-shrink-0 cursor-zoom-in" onclick={() => onLightboxOpen(img.path)}>
								<img
									src={img.path}
									alt={img.caption ?? ''}
									class="h-24 w-24 rounded-lg object-cover border border-slate-700 hover:border-emerald-500 transition-colors"
								/>
							</button>
						{/each}
					</div>
				{/if}

				{#if item.container_path}
					<p class="mb-1 text-xs text-slate-500">Location</p>
					<p class="mb-3 text-sm text-slate-300 break-words">{item.container_path}</p>
				{/if}

				{#if item.description}
					<p class="mb-1 text-xs text-slate-500">Description</p>
					<p class="mb-3 text-sm text-slate-300 whitespace-pre-wrap">{item.description}</p>
				{/if}

				<div class="mb-3 grid grid-cols-2 gap-2 text-xs">
					{#if item.system_barcode}
						<div
							><span class="text-slate-500">Barcode:</span> <span class="font-mono text-slate-300"
								>{item.system_barcode}</span
							></div
						>
					{/if}
					{#if item.is_fungible && item.fungible_quantity !== null}
						<div
							><span class="text-slate-500">Qty:</span> <span class="text-slate-300"
								>{item.fungible_quantity}{item.fungible_unit ? ' ' + item.fungible_unit : ''}</span
							></div
						>
					{/if}
					{#if item.category}
						<div><span class="text-slate-500">Category:</span> <span class="text-slate-300">{item.category}</span></div>
					{/if}
				</div>

				<a href="/browse/item/{item.id}" class="btn btn-secondary w-full text-center">Open full page</a>
			{/if}
		</div>
	</div>
{/if}
