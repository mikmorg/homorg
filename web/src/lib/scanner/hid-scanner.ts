/**
 * HID keyboard wedge scanner.
 *
 * Most barcode scanners act as USB or Bluetooth HID devices that emit
 * rapid keydown events followed by Enter. We detect this by:
 *   1. Buffering keystrokes that arrive within MIN_CHARS_PER_SECOND milliseconds.
 *   2. Emitting when Enter is pressed after accumulating ≥ MIN_BARCODE_LENGTH characters.
 *
 * To avoid capturing normal keyboard typing we require a minimum burst speed.
 */

import { BaseScanner } from './scanner.js';

const MAX_INTER_CHAR_MS = 50;  // scanners emit chars faster than humans can type
const MIN_BARCODE_LENGTH = 4;
const DEDUPE_MS = 1500;        // suppress re-emission of the same barcode within this window

export class HidScanner extends BaseScanner {
	private buffer = '';
	private lastKeyTime = 0;
	private readonly lastSeen = new Map<string, number>();
	private readonly handler: (e: KeyboardEvent) => void;

	constructor() {
		super();

		this.handler = (e: KeyboardEvent) => {
			// Skip events firing inside text inputs / textareas unless intentional
			const target = e.target as HTMLElement | null;
			if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA')) {
				if (!(target as HTMLInputElement).dataset.scannerTarget) return;
			}

			const now = performance.now();
			const elapsed = now - this.lastKeyTime;
			this.lastKeyTime = now;

			if (e.key === 'Enter') {
				const barcode = this.buffer.trim();
				this.buffer = '';
				if (barcode.length >= MIN_BARCODE_LENGTH) {
					const now = performance.now();
					const last = this.lastSeen.get(barcode) ?? -Infinity;
					if (now - last > DEDUPE_MS) {
						this.lastSeen.set(barcode, now);
						this.emit(barcode, 'hid');
					}
				}
				return;
			}

			if (elapsed > MAX_INTER_CHAR_MS && this.buffer.length > 0) {
				// Gap too large — this is keyboard typing, not a scanner burst
				this.buffer = '';
			}

			if (e.key.length === 1) {
				this.buffer += e.key;
			}
		};
	}

	override async start() {
		await super.start();
		window.addEventListener('keydown', this.handler, { capture: true });
	}

	override stop() {
		super.stop();
		window.removeEventListener('keydown', this.handler, { capture: true });
		this.buffer = '';
		this.lastSeen.clear();
	}
}
