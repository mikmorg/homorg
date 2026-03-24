<script lang="ts">
	import { toasts, dismissToast, toast } from '$stores/toast.js';
	import { api } from '$api/client.js';

	async function handleUndo(toastId: number, eventId: string) {
		dismissToast(toastId);
		try {
			await api.undo.single(eventId);
		} catch {
			toast('Undo failed', 'error');
		}
	}
</script>

<div class="fixed top-4 left-1/2 -translate-x-1/2 z-[100] flex flex-col gap-2 pointer-events-none w-[90vw] max-w-sm">
	{#each $toasts as msg (msg.id)}
		<div
			class="pointer-events-auto rounded-lg px-4 py-2.5 text-sm font-medium shadow-lg animate-in flex items-center gap-2
				{msg.type === 'success' ? 'bg-emerald-600 text-white' : ''}
				{msg.type === 'error' ? 'bg-red-600 text-white' : ''}
				{msg.type === 'info' ? 'bg-slate-700 text-slate-100' : ''}"
		>
			<span class="flex-1">{msg.text}</span>
			{#if msg.undoEventId}
				<button
					class="flex-shrink-0 rounded px-2 py-0.5 text-xs font-bold bg-white/20 hover:bg-white/30 transition-colors"
					on:click={() => handleUndo(msg.id, msg.undoEventId ?? '')}
				>
					Undo
				</button>
			{/if}
		</div>
	{/each}
</div>

<style>
	.animate-in {
		animation: slide-in 0.2s ease-out;
	}
	@keyframes slide-in {
		from { opacity: 0; transform: translateY(-0.5rem); }
		to { opacity: 1; transform: translateY(0); }
	}
</style>
