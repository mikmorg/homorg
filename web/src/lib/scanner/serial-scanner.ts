/**
 * Web Serial scanner for Bluetooth SPP / USB COM-port barcode scanners.
 *
 * Supported on Chrome 117+ (including Chrome on Android).
 * Requires a user gesture to call requestPort(), then a SerialPort is held
 * open and bytes are read with a ReadableStreamDefaultReader.
 *
 * Most scanners terminate each barcode with CR (\r), CRLF (\r\n), or LF (\n).
 * We buffer incoming bytes and emit on any line terminator.
 */

import { BaseScanner } from './scanner.js';

const MIN_BARCODE_LENGTH = 4;

export function isSerialSupported(): boolean {
	return typeof navigator !== 'undefined' && 'serial' in navigator;
}

export class SerialScanner extends BaseScanner {
	private port: SerialPort | null = null;
	private reader: ReadableStreamDefaultReader<Uint8Array> | null = null;
	private decoder = new TextDecoder();
	private lineBuffer = '';
	private running = false;

	/** Call from a user gesture. Opens port picker and connects. */
	override async start(): Promise<void> {
		if (!isSerialSupported()) {
			throw new Error('Web Serial API not supported in this browser.');
		}
		this.port = await (navigator as Navigator & { serial: { requestPort: () => Promise<SerialPort> } }).serial.requestPort();
		await this.port.open({ baudRate: 9600 });
		await super.start();
		this.running = true;
		this.readLoop().catch(console.error);
	}

	private async readLoop() {
		if (!this.port?.readable) return;
		this.reader = this.port.readable.getReader();
		try {
			while (this.running) {
				const { value, done } = await this.reader.read();
				if (done) break;
				const chunk = this.decoder.decode(value, { stream: true });
				this.lineBuffer += chunk;
				let idx: number;
				while ((idx = this.lineBuffer.search(/\r?\n|\r/)) !== -1) {
					const line = this.lineBuffer.slice(0, idx).trim();
					this.lineBuffer = this.lineBuffer.slice(idx + 1);
					if (line.length >= MIN_BARCODE_LENGTH) {
						this.emit(line, 'serial');
					}
				}
			}
		} catch {
			// Port disconnected or user revoked permission
		} finally {
			this.reader?.releaseLock();
			this.reader = null;
		}
	}

	override stop() {
		super.stop();
		this.running = false;
		this.reader?.cancel().catch(() => {});
		this.port?.close().catch(() => {});
		this.port = null;
		this.lineBuffer = '';
	}
}
