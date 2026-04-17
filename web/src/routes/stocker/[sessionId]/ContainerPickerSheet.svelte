<script lang="ts">
	import type { ItemSummary } from '$api/types.js';
	import type { RecentContainer } from '$stores/recentContainers.js';

	interface Props {
		show: boolean;
		query: string;
		results: ItemSummary[];
		loading: boolean;
		recents: RecentContainer[];
		onClose: () => void;
		onQueryChange: (query: string) => void;
		onInput: (query: string) => void;
		onSelectContainer: (container: ItemSummary | RecentContainer) => void;
	}

	const {
		show = false,
		query = '',
		results = [],
		loading = false,
		recents = [],
		onClose,
		onQueryChange,
		onInput,
		onSelectContainer
	}: Props = $props();
</script>

{#if show}
	<div class="fixed inset-0 z-50 flex flex-col bg-slate-950">
		<div class="flex items-center gap-2 border-b border-slate-800 px-3 py-2">
			<button class="btn btn-icon text-slate-400" onclick={onClose} aria-label="Close">
				<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M18 6L6 18M6 6l12 12" />
				</svg>
			</button>
			<input
				class="input flex-1"
				placeholder="Search containers…"
				value={query}
				onchange={(e) => onQueryChange((e.target as HTMLInputElement).value)}
				oninput={(e) => onInput((e.target as HTMLInputElement).value)}
			/>
		</div>

		<div class="flex-1 overflow-y-auto p-3">
			{#if loading}
				<div class="flex h-16 items-center justify-center">
					<div class="h-5 w-5 animate-spin rounded-full border-2 border-slate-600 border-t-indigo-500"></div>
				</div>
			{:else if !query.trim()}
				<!-- No query: show recents then all containers -->
				{#if recents.length > 0}
					<p class="mb-2 px-1 text-xs font-medium uppercase tracking-wide text-slate-500">Recent</p>
					<div class="space-y-1 mb-4">
						{#each recents as rc (rc.id)}
							<button
								class="flex w-full items-center gap-3 rounded-lg px-3 py-3 text-left transition-colors hover:bg-slate-800"
								onclick={() => onSelectContainer(rc)}
							>
								<div class="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-md bg-indigo-500/20 text-indigo-400">
									<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
										<path d="M21 8a2 2 0 0 0-1.5-1.937A2 2 0 0 0 18 5.5V5a2 2 0 0 0-2-2H8a2 2 0 0 0-2 2v.5A2 2 0 0 0 4.5 6.063 2 2 0 0 0 3 8v9a3 3 0 0 0 3 3h12a3 3 0 0 0 3-3z" />
									</svg>
								</div>
								<div class="min-w-0">
									<p class="truncate font-medium text-slate-100">{rc.name}</p>
									{#if rc.parent_name}
										<p class="truncate text-xs text-slate-500">in {rc.parent_name}</p>
									{/if}
								</div>
							</button>
						{/each}
					</div>
				{/if}
				{#if results.length > 0}
					<p class="mb-2 px-1 text-xs font-medium uppercase tracking-wide text-slate-500">All containers</p>
					<div class="space-y-1">
						{#each results as item (item.id)}
							<button
								class="flex w-full items-center gap-3 rounded-lg px-3 py-3 text-left transition-colors hover:bg-slate-800"
								onclick={() => onSelectContainer(item)}
							>
								<div class="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-md bg-indigo-500/20 text-indigo-400 text-xs">
									<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
										<path d="M21 8a2 2 0 0 0-1.5-1.937A2 2 0 0 0 18 5.5V5a2 2 0 0 0-2-2H8a2 2 0 0 0-2 2v.5A2 2 0 0 0 4.5 6.063 2 2 0 0 0 3 8v9a3 3 0 0 0 3 3h12a3 3 0 0 0 3-3z" />
									</svg>
								</div>
								<div class="min-w-0">
									<p class="truncate font-medium text-slate-100">{item.name ?? 'Unnamed'}</p>
									{#if item.parent_name}
										<p class="truncate text-xs text-slate-500">in {item.parent_name}</p>
									{:else if item.system_barcode}
										<p class="text-xs text-slate-400 font-mono">{item.system_barcode}</p>
									{/if}
								</div>
							</button>
						{/each}
					</div>
				{/if}
			{:else if results.length === 0}
				<p class="py-8 text-center text-sm text-slate-500">No containers found</p>
			{:else}
				<div class="space-y-1">
					{#each results as item (item.id)}
						<button
							class="flex w-full items-center gap-3 rounded-lg px-3 py-3 text-left transition-colors hover:bg-slate-800"
							onclick={() => onSelectContainer(item)}
						>
							<div class="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-md bg-indigo-500/20 text-indigo-400 text-xs">
								<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
									<path d="M21 8a2 2 0 0 0-1.5-1.937A2 2 0 0 0 18 5.5V5a2 2 0 0 0-2-2H8a2 2 0 0 0-2 2v.5A2 2 0 0 0 4.5 6.063 2 2 0 0 0 3 8v9a3 3 0 0 0 3 3h12a3 3 0 0 0 3-3z" />
								</svg>
							</div>
							<div class="min-w-0">
								<p class="truncate font-medium text-slate-100">{item.name ?? 'Unnamed'}</p>
								{#if item.container_path}
									<p class="truncate text-xs text-slate-500">{item.container_path}</p>
								{:else if item.system_barcode}
									<p class="text-xs text-slate-400 font-mono">{item.system_barcode}</p>
								{/if}
							</div>
						</button>
					{/each}
				</div>
			{/if}
		</div>
	</div>
{/if}
