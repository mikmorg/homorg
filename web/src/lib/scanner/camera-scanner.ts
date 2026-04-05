/**
 * Camera-based barcode scanner.
 *
 * Tries the native BarcodeDetector API (Chrome 83+, requires HTTPS) first,
 * then falls back to jsQR (pure-JS QR decoder, works over plain HTTP).
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
	private canvas: HTMLCanvasElement | null = null;
	private ctx: CanvasRenderingContext2D | null = null;
	private useJsQr = false;
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
		if (isBarcodeDetectorSupported()) {
			const formats = await BarcodeDetector.getSupportedFormats();
			this.detector = new BarcodeDetector({ formats });
			this.useJsQr = false;
		} else {
			// Fall back to jsQR — works over HTTP and on all browsers with getUserMedia
			this.useJsQr = true;
			this.canvas = document.createElement('canvas');
			this.ctx = this.canvas.getContext('2d');
		}

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
		if (this.videoElement.readyState < 2) return;
		const now = Date.now();

		if (!this.useJsQr && this.detector) {
			const results = await this.detector.detect(this.videoElement);
			for (const result of results) {
				this.maybeEmit(result.rawValue.trim(), now, result.format);
			}
			return;
		}

		// jsQR path
		if (!this.canvas || !this.ctx) return;
		const { videoWidth: w, videoHeight: h } = this.videoElement;
		if (w === 0 || h === 0) return;

		this.canvas.width = w;
		this.canvas.height = h;
		this.ctx.drawImage(this.videoElement, 0, 0, w, h);
		const imageData = this.ctx.getImageData(0, 0, w, h);

		const jsQR = (await import('jsqr')).default;
		const result = jsQR(imageData.data, w, h);
		if (result?.data) {
			this.maybeEmit(result.data.trim(), now, 'qr_code');
		}
	}

	private maybeEmit(barcode: string, now: number, format?: string) {
		if (!barcode) return;
		const last = this.lastSeen.get(barcode) ?? 0;
		if (now - last < DEBOUNCE_MS) return;
		this.lastSeen.set(barcode, now);
		this.emit(barcode, 'camera', format);
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
		this.canvas = null;
		this.ctx = null;
	}
}
