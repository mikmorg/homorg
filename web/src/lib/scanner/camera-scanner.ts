/**
 * Camera-based barcode scanner using the BarcodeDetector API (Chrome 83+).
 *
 * Provides a <video> element that callers should mount in the DOM, and emits
 * scan events via the BaseScanner interface.
 *
 * Usage:
 *   const cam = new CameraScanner();
 *   await cam.start();
 *   document.body.appendChild(cam.videoElement);
 *   cam.onScan(e => console.log(e.barcode));
 *   // later:
 *   cam.stop();
 */

import { BaseScanner } from './scanner.js';

export function isBarcodeDetectorSupported(): boolean {
	return typeof window !== 'undefined' && 'BarcodeDetector' in window;
}

const SCAN_INTERVAL_MS = 250;
const DEBOUNCE_MS = 1500;  // ignore the same barcode for this long after emitting

declare global {
	class BarcodeDetector {
		constructor(options?: { formats: string[] });
		detect(image: ImageBitmapSource): Promise<{ rawValue: string; format: string }[]>;
		static getSupportedFormats(): Promise<string[]>;
	}
}

export class CameraScanner extends BaseScanner {
	readonly videoElement: HTMLVideoElement;
	private stream: MediaStream | null = null;
	private detector: BarcodeDetector | null = null;
	private intervalId: ReturnType<typeof setInterval> | null = null;
	private lastSeen = new Map<string, number>();

	constructor() {
		super();
		this.videoElement = document.createElement('video');
		this.videoElement.setAttribute('playsinline', '');
		this.videoElement.setAttribute('muted', '');
		this.videoElement.style.width = '100%';
		this.videoElement.style.maxWidth = '480px';
	}

	override async start(): Promise<void> {
		if (!isBarcodeDetectorSupported()) {
			throw new Error('BarcodeDetector API not supported in this browser.');
		}

		const formats = await BarcodeDetector.getSupportedFormats();
		this.detector = new BarcodeDetector({ formats });

		this.stream = await navigator.mediaDevices.getUserMedia({
			video: { facingMode: 'environment' }
		});
		this.videoElement.srcObject = this.stream;
		await this.videoElement.play();
		await super.start();

		this.intervalId = setInterval(() => {
			this.detect().catch(() => {});
		}, SCAN_INTERVAL_MS);
	}

	private async detect() {
		if (!this.detector || this.videoElement.readyState < 2) return;
		const results = await this.detector.detect(this.videoElement);
		const now = Date.now();

		for (const result of results) {
			const barcode = result.rawValue.trim();
			if (!barcode) continue;
			const last = this.lastSeen.get(barcode) ?? 0;
			if (now - last < DEBOUNCE_MS) continue;
			this.lastSeen.set(barcode, now);
			this.emit(barcode, 'camera');
		}
	}

	override stop() {
		super.stop();
		if (this.intervalId !== null) {
			clearInterval(this.intervalId);
			this.intervalId = null;
		}
		this.stream?.getTracks().forEach((t) => t.stop());
		this.stream = null;
		this.videoElement.srcObject = null;
		this.lastSeen.clear();
	}
}
