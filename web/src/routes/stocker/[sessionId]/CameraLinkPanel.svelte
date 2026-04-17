<script lang="ts">
	import type { CameraToken } from '$api/types.js';

	interface Props {
		show: boolean;
		tokens: CameraToken[];
		loading: boolean;
		error: string;
		deviceName: string;
		qrCodes: Record<string, string>;
		onClose: () => void;
		onDeviceNameChange: (name: string) => void;
		onCreateLink: () => void;
		onRevokeLink: (tokenId: string) => void;
		getCameraUrl: (token: string) => string;
	}

	const {
		show = false,
		tokens = [],
		loading = false,
		error = '',
		deviceName = '',
		qrCodes = {},
		onClose,
		onDeviceNameChange,
		onCreateLink,
		onRevokeLink,
		getCameraUrl
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
		<div
			class="rounded-t-2xl bg-slate-900 p-4 pb-8 max-h-[80vh] overflow-y-auto"
			role="dialog"
			aria-modal="true"
			aria-labelledby="camera-link-title"
			tabindex="-1"
		>
			<div class="mb-4 flex items-center justify-between">
				<h2 id="camera-link-title" class="text-base font-semibold text-slate-100">📷 Remote Camera</h2>
				<button class="btn btn-icon text-slate-400" onclick={onClose} aria-label="Close">
					<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
						<path d="M18 6L6 18M6 6l12 12" />
					</svg>
				</button>
			</div>

			<p class="mb-3 text-sm text-slate-400">
				Link a remote camera device (e.g. Android phone) to this session. Photos taken will auto-attach to the
				most recently scanned item.
			</p>

			{#if error}
				<div class="mb-3 rounded-lg bg-red-950 px-3 py-2 text-sm text-red-300 border border-red-800">
					{error}
				</div>
			{/if}

			<!-- Create new link -->
			<div class="mb-4 space-y-2">
				<div class="flex gap-2">
					<input
						class="input flex-1 text-sm"
						placeholder="Device name (optional)"
						value={deviceName}
						onchange={(e) => onDeviceNameChange((e.target as HTMLInputElement).value)}
						disabled={loading}
					/>
					<button class="btn btn-primary text-sm px-3" onclick={onCreateLink} disabled={loading}>
						{loading ? '…' : 'Link'}
					</button>
				</div>
			</div>

			<!-- Active links -->
			{#if tokens.length > 0}
				<div class="space-y-3">
					{#each tokens as ct (ct.id)}
						<div class="rounded-lg border border-slate-700 bg-slate-800/50 p-3">
							<div class="flex items-start justify-between gap-2 mb-2">
								<div>
									<p class="text-sm font-medium text-slate-200">
										{ct.device_name ?? 'Camera device'}
									</p>
									<p class="text-xs text-slate-400">
										Expires {new Date(ct.expires_at).toLocaleString(undefined, {
											month: 'short',
											day: 'numeric',
											hour: '2-digit',
											minute: '2-digit'
										})}
									</p>
								</div>
								<button
									class="text-xs text-red-400 hover:text-red-300"
									onclick={() => onRevokeLink(ct.id)}
								>
									Revoke
								</button>
							</div>

							<!-- QR code for the Homorg Camera app -->
							{#if qrCodes[ct.id]}
								<div class="flex flex-col items-center gap-1 my-2">
									<img
										src={qrCodes[ct.id]}
										alt="QR code — scan with Homorg Camera app"
										class="h-48 w-48 rounded"
									/>
									<p class="text-xs text-slate-500">Scan with Homorg Camera app</p>
								</div>
							{/if}

							<!-- Token URL for manual entry -->
							<div class="rounded bg-slate-950 p-2">
								<p class="text-xs text-slate-500 mb-1">Or paste this URL manually:</p>
								<code class="block text-xs text-emerald-400 break-all select-all">
									{getCameraUrl(ct.token)}/upload
								</code>
							</div>
						</div>
					{/each}
				</div>
			{:else}
				<p class="text-sm text-slate-500 text-center py-4">No active camera links</p>
			{/if}
		</div>
	</div>
{/if}
