<script lang="ts">
	import { scannerState } from '$scanner/index.js';
	import { startSerialScanner, startCameraScanner, startHidScanner } from '$scanner/index.js';

	interface Props {
		show: boolean;
		onClose: () => void;
		onPickCamera: () => void;
	}

	const { show = false, onClose, onPickCamera }: Props = $props();
</script>

{#if show}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="fixed inset-0 z-50 flex flex-col justify-end bg-black/60"
		onclick={(e) => {
			if (e.target === e.currentTarget) {
				onClose();
			}
		}}
		onkeydown={(e) => e.key === 'Escape' && onClose()}
	>
		<div
			class="rounded-t-2xl bg-slate-900 p-4 pb-8"
			role="dialog"
			aria-modal="true"
			aria-labelledby="scanner-settings-title"
		>
			<h2 id="scanner-settings-title" class="mb-4 text-base font-semibold text-slate-100">
				Scanner source
			</h2>
			<div class="space-y-2">
				<button
					class="btn w-full justify-start gap-3"
					class:btn-primary={$scannerState.source === 'hid'}
					class:btn-secondary={$scannerState.source !== 'hid'}
					onclick={() => {
						startHidScanner();
						onClose();
					}}
				>
					<span class="text-lg">⌨️</span>
					<span>HID keyboard wedge <span class="text-xs opacity-70">(USB/BT HID)</span></span>
				</button>
				<button
					class="btn w-full justify-start gap-3"
					class:btn-primary={$scannerState.source === 'serial'}
					class:btn-secondary={$scannerState.source !== 'serial'}
					onclick={() => {
						startSerialScanner();
						onClose();
					}}
				>
					<span class="text-lg">🔵</span>
					<span>Bluetooth SPP / USB Serial <span class="text-xs opacity-70">(Chrome 117+)</span></span>
				</button>
				<button
					class="btn w-full justify-start gap-3"
					class:btn-primary={$scannerState.source === 'camera'}
					class:btn-secondary={$scannerState.source !== 'camera'}
					onclick={onPickCamera}
				>
					<span class="text-lg">📷</span>
					<span>Camera <span class="text-xs opacity-70">(BarcodeDetector API)</span></span>
				</button>
			</div>

			{#if $scannerState.errorMessage}
				<p class="mt-3 text-sm text-red-400">{$scannerState.errorMessage}</p>
			{/if}
		</div>
	</div>
{/if}
