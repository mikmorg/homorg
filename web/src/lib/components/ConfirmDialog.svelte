<script lang="ts">
	export let open = false;
	export let title = 'Are you sure?';
	export let message = '';
	export let confirmLabel = 'Confirm';
	export let cancelLabel = 'Cancel';
	export let destructive = false;
	export let loading = false;
	export let onConfirm: () => void;
	export let onCancel: (() => void) | undefined = undefined;

	function handleCancel() {
		open = false;
		onCancel?.();
	}

	function handleConfirm() {
		onConfirm();
	}
</script>

{#if open}
<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 px-4" on:click|self={handleCancel}>
	<div class="w-full max-w-sm rounded-2xl bg-slate-900 border border-slate-800 p-5">
		<h3 class="text-base font-semibold text-slate-100">{title}</h3>
		{#if message}
			<p class="mt-2 text-sm text-slate-400">{message}</p>
		{/if}
		<div class="mt-4 flex gap-3">
			<button class="btn btn-secondary flex-1" on:click={handleCancel} disabled={loading}>
				{cancelLabel}
			</button>
			<button
				class="btn flex-1 {destructive ? 'btn-danger' : 'btn-primary'}"
				on:click={handleConfirm}
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
