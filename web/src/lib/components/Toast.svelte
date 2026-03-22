<script lang="ts">
	import { writable } from 'svelte/store';

	interface ToastMessage {
		id: number;
		text: string;
		type: 'success' | 'error' | 'info';
	}

	let idCounter = 0;
	export const toasts = writable<ToastMessage[]>([]);

	export function toast(text: string, type: ToastMessage['type'] = 'success', duration = 3000) {
		const id = ++idCounter;
		toasts.update((t) => [...t, { id, text, type }]);
		setTimeout(() => {
			toasts.update((t) => t.filter((m) => m.id !== id));
		}, duration);
	}
</script>

<div class="fixed top-4 left-1/2 -translate-x-1/2 z-[100] flex flex-col gap-2 pointer-events-none">
	{#each $toasts as msg (msg.id)}
		<div
			class="pointer-events-auto rounded-lg px-4 py-2.5 text-sm font-medium shadow-lg animate-in
				{msg.type === 'success' ? 'bg-emerald-600 text-white' : ''}
				{msg.type === 'error' ? 'bg-red-600 text-white' : ''}
				{msg.type === 'info' ? 'bg-slate-700 text-slate-100' : ''}"
		>
			{msg.text}
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
