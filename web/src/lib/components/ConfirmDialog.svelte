<script lang="ts">
	let {
		open = $bindable(false),
		title = 'Are you sure?',
		message = '',
		confirmLabel = 'Confirm',
		cancelLabel = 'Cancel',
		destructive = false,
		loading = false,
		onConfirm,
		onCancel = undefined
	}: {
		open: boolean;
		title?: string;
		message?: string;
		confirmLabel?: string;
		cancelLabel?: string;
		destructive?: boolean;
		loading?: boolean;
		onConfirm: () => void;
		onCancel?: () => void;
	} = $props();

	function handleCancel() {
		open = false;
		onCancel?.();
	}

	function handleConfirm() {
		onConfirm();
	}
</script>

{#if open}
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 px-4"
	onclick={(e) => { if (e.target === e.currentTarget) handleCancel(); }}
	onkeydown={(e) => e.key === 'Escape' && handleCancel()}
>
	<div class="w-full max-w-sm rounded-2xl bg-slate-900 border border-slate-800 p-5" role="dialog" aria-modal="true" aria-labelledby="confirm-dialog-title">
		<h3 id="confirm-dialog-title" class="text-base font-semibold text-slate-100">{title}</h3>
		{#if message}
			<p class="mt-2 text-sm text-slate-400">{message}</p>
		{/if}
		<div class="mt-4 flex gap-3">
			<button class="btn btn-secondary flex-1" onclick={handleCancel} disabled={loading}>
				{cancelLabel}
			</button>
			<button
				class="btn flex-1 {destructive ? 'btn-danger' : 'btn-primary'}"
				onclick={handleConfirm}
				disabled={loading}
			>
				{#if loading}
					<span class="h-4 w-4 animate-spin rounded-full border-2 border-white/30 border-t-white"></span>
				{:else}
					{confirmLabel}
				{/if}
			</button>
		</div>
	</div>
</div>
{/if}
