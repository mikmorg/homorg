<script lang="ts">
	interface ScanLogEntry {
		id: number;
		barcode: string;
		type: 'success' | 'context' | 'create' | 'error';
		message: string;
		item?: any;
		itemId?: string;
		itemName?: string;
		parentId?: string;
		parentName?: string;
		imageUrl?: string;
		timestamp: number;
	}

	interface Props {
		entries: ScanLogEntry[];
		nameCache: Record<string, string>;
		lightboxUrl: string | null;
		onLightboxOpen: (url: string) => void;
	}

	const { entries = [], nameCache = {}, lightboxUrl, onLightboxOpen }: Props = $props();

	function entryText(e: ScanLogEntry): string {
		if (!e.itemId) return e.message;
		const name = nameCache[e.itemId] ?? e.itemName ?? '';
		const parent = (e.parentId && nameCache[e.parentId]) ?? e.parentName;
		if (!name) return e.message;
		if (e.imageUrl) return `Photo added: ${name}`;
		if (e.type === 'create') return parent ? `Created: ${name} → ${parent}` : `Created: ${name}`;
		if (e.message.startsWith('Moved: ') || e.message.startsWith('Moved container: ')) {
			const prefix = e.message.startsWith('Moved container: ') ? 'Moved container' : 'Moved';
			return parent ? `${prefix}: ${name} → ${parent}` : `${prefix}: ${name}`;
		}
		if (e.message.startsWith('Queued: ')) {
			return parent ? `Queued: ${name} → ${parent}` : `Queued: ${name}`;
		}
		if (e.message.startsWith('Updated: ')) return `Updated: ${name}`;
		if (e.message.startsWith('Deleted: ')) return `Deleted: ${name}`;
		return e.message;
	}

	function logClass(type: ScanLogEntry['type']) {
		return `scan-line-${type}`;
	}

	function logTime(ts: number): string {
		const d = new Date(ts);
		return d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit', second: '2-digit' });
	}
</script>

<!-- Scan log display -->
<div class="flex-1 overflow-y-auto font-mono text-sm">
	{#if entries.length === 0}
		<div class="flex h-32 flex-col items-center justify-center gap-1 text-slate-600">
			<p>Waiting for scans…</p>
			<p class="text-xs">Scan a container, then items</p>
		</div>
	{:else}
		{#each entries as entry (entry.id)}
			<div class="scan-line {logClass(entry.type)} flex items-center gap-2 px-4 py-2">
				<span class="w-14 flex-shrink-0 text-[10px] tabular-nums text-slate-600">{logTime(entry.timestamp)}</span>
				{#if entry.imageUrl}
					<button
						class="flex-shrink-0 cursor-zoom-in"
						onclick={() => onLightboxOpen(entry.imageUrl ?? '')}
					>
						<img
							src={entry.imageUrl}
							alt=""
							class="h-8 w-8 rounded object-cover border border-slate-700 hover:border-emerald-500 transition-colors"
						/>
					</button>
				{/if}
				{#if entry.itemId}
					<a
						href="/browse/item/{entry.itemId}"
						class="flex-1 truncate hover:text-emerald-400 transition-colors"
					>
						{entryText(entry)}
					</a>
				{:else}
					<span class="flex-1 truncate">{entryText(entry)}</span>
				{/if}
			</div>
		{/each}
	{/if}
</div>
