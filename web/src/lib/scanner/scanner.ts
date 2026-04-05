/**
 * Unified scanner abstraction.
 * Emits `scan` events with a barcode string regardless of the underlying source.
 *
 * Three-tier priority:
 *   1. HID keyboard wedge  — zero permissions, universal, primary
 *   2. Web Serial (BT SPP) — Chrome 117+, requires user gesture + permission
 *   3. Camera             — BarcodeDetector API or polyfill, fallback
 */

export interface ScanEvent {
	barcode: string;
	source: 'hid' | 'serial' | 'camera';
	/** Barcode format as reported by BarcodeDetector (e.g. 'ean_13', 'upc_a', 'qr_code'). */
	format?: string;
}

type ScanHandler = (event: ScanEvent) => void;

export interface Scanner {
	/** Start listening for scans. */
	start(): Promise<void>;
	/** Stop listening. */
	stop(): void;
	/** Register a scan handler. */
	onScan(handler: ScanHandler): () => void;
	/** True if this scanner source is currently active. */
	readonly active: boolean;
}

/**
 * A simple EventTarget-based emitter for scan events.
 * Extend this class instead of reimplementing subscription management.
 */
export class BaseScanner extends EventTarget implements Scanner {
	private _active = false;

	get active() {
		return this._active;
	}

	protected setActive(v: boolean) {
		this._active = v;
	}

	// eslint-disable-next-line @typescript-eslint/no-unused-vars
	async start(): Promise<void> {
		this.setActive(true);
	}

	stop(): void {
		this.setActive(false);
	}

	onScan(handler: ScanHandler): () => void {
		const listener = (e: Event) => handler((e as CustomEvent<ScanEvent>).detail);
		this.addEventListener('scan', listener);
		return () => this.removeEventListener('scan', listener);
	}

	protected emit(barcode: string, source: ScanEvent['source'], format?: string) {
		this.dispatchEvent(new CustomEvent<ScanEvent>('scan', { detail: { barcode, source, format } }));
	}
}
