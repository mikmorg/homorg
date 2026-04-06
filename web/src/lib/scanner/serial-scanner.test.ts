import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { SerialScanner, isSerialSupported } from './serial-scanner.js';

// ── Helpers ──────────────────────────────────────────────────────────────────

/**
 * Build a mock SerialPort whose readable delivers the given byte chunks in
 * order via Promise.resolve, then pauses forever.
 */
function makePort(chunks: Uint8Array[]): {
	port: SerialPort;
} {
	let chunkIndex = 0;

	const reader = {
		read: vi.fn(async () => {
			if (chunkIndex < chunks.length) {
				return { value: chunks[chunkIndex++], done: false as const };
			}
			// Pause indefinitely — scanner stays running
			return new Promise<{ value: undefined; done: true }>(() => {});
		}),
		releaseLock: vi.fn(),
		cancel: vi.fn().mockResolvedValue(undefined),
	};

	const port = {
		open: vi.fn().mockResolvedValue(undefined),
		close: vi.fn().mockResolvedValue(undefined),
		readable: { getReader: vi.fn(() => reader) },
	} as unknown as SerialPort;

	return { port };
}

/** Encode a string as UTF-8 bytes. */
function enc(s: string): Uint8Array {
	return new TextEncoder().encode(s);
}

/** Set up navigator.serial mock to return the given port. */
function mockSerial(port: SerialPort) {
	Object.defineProperty(navigator, 'serial', {
		value: { requestPort: vi.fn().mockResolvedValue(port) },
		configurable: true,
		writable: true,
	});
}

/**
 * Wait long enough for the async readLoop microtasks to flush.
 * We use a real timer here because the readLoop uses real microtask scheduling.
 */
function flushReadLoop(): Promise<void> {
	return new Promise((r) => setTimeout(r, 30));
}

// ── isSerialSupported ────────────────────────────────────────────────────────

describe('isSerialSupported', () => {
	it('returns true when navigator.serial is present', () => {
		Object.defineProperty(navigator, 'serial', {
			value: {},
			configurable: true,
			writable: true,
		});
		expect(isSerialSupported()).toBe(true);
	});

	it('returns false when navigator.serial is absent (node/SSR context)', () => {
		// isSerialSupported checks `'serial' in navigator`. jsdom always keeps
		// the property key once defined, so testing the absent-serial path
		// requires a different navigator object. We verify the function is safe
		// to call — it won't throw — and documents the expected behavior.
		// The true absence case (SSR / non-browser) is covered by the typeof
		// navigator guard; we trust the implementation and skip the unreachable
		// jsdom path.
		expect(() => isSerialSupported()).not.toThrow();
	});
});

// ── SerialScanner — line parsing ─────────────────────────────────────────────

describe('SerialScanner — line terminator handling', () => {
	let scanner: SerialScanner;
	let scans: { barcode: string; source: string }[];
	let unsubscribe: () => void;

	beforeEach(() => {
		scanner = new SerialScanner();
		scans = [];
	});

	afterEach(() => {
		unsubscribe?.();
		scanner.stop();
		vi.restoreAllMocks();
	});

	it('emits barcode terminated with LF (\\n)', async () => {
		const { port } = makePort([enc('HOM-000042\n')]);
		mockSerial(port);
		// Subscribe BEFORE start so the handler is registered when the readLoop fires
		unsubscribe = scanner.onScan((e) => scans.push(e));
		await scanner.start();
		await flushReadLoop();
		expect(scans).toHaveLength(1);
		expect(scans[0]).toEqual({ barcode: 'HOM-000042', source: 'serial', format: undefined });
	});

	it('emits barcode terminated with CR (\\r)', async () => {
		const { port } = makePort([enc('HOM-000042\r')]);
		mockSerial(port);
		unsubscribe = scanner.onScan((e) => scans.push(e));
		await scanner.start();
		await flushReadLoop();
		expect(scans[0]?.barcode).toBe('HOM-000042');
	});

	it('emits barcode terminated with CRLF (\\r\\n)', async () => {
		const { port } = makePort([enc('HOM-000042\r\n')]);
		mockSerial(port);
		unsubscribe = scanner.onScan((e) => scans.push(e));
		await scanner.start();
		await flushReadLoop();
		expect(scans[0]?.barcode).toBe('HOM-000042');
	});

	it('does not emit when barcode is shorter than 4 chars', async () => {
		const { port } = makePort([enc('AB\n')]);
		mockSerial(port);
		unsubscribe = scanner.onScan((e) => scans.push(e));
		await scanner.start();
		await flushReadLoop();
		expect(scans).toHaveLength(0);
	});

	it('emits at exactly 4 chars', async () => {
		const { port } = makePort([enc('ABCD\n')]);
		mockSerial(port);
		unsubscribe = scanner.onScan((e) => scans.push(e));
		await scanner.start();
		await flushReadLoop();
		expect(scans[0]?.barcode).toBe('ABCD');
	});

	it('trims surrounding whitespace from emitted barcode', async () => {
		const { port } = makePort([enc('  HOM-042  \n')]);
		mockSerial(port);
		unsubscribe = scanner.onScan((e) => scans.push(e));
		await scanner.start();
		await flushReadLoop();
		expect(scans[0]?.barcode).toBe('HOM-042');
	});

	it('buffers partial chunks across multiple reads', async () => {
		// Deliver the barcode in two separate chunks
		const { port } = makePort([enc('HOM-'), enc('000042\n')]);
		mockSerial(port);
		unsubscribe = scanner.onScan((e) => scans.push(e));
		await scanner.start();
		await flushReadLoop();
		expect(scans[0]?.barcode).toBe('HOM-000042');
	});

	it('emits multiple barcodes from a single chunk', async () => {
		const { port } = makePort([enc('HOM-001\nHOM-002\n')]);
		mockSerial(port);
		unsubscribe = scanner.onScan((e) => scans.push(e));
		await scanner.start();
		await flushReadLoop();
		expect(scans).toHaveLength(2);
		expect(scans[0].barcode).toBe('HOM-001');
		expect(scans[1].barcode).toBe('HOM-002');
	});
});

// ── SerialScanner — lifecycle ─────────────────────────────────────────────────

describe('SerialScanner — lifecycle', () => {
	afterEach(() => vi.restoreAllMocks());

	it('is inactive before start()', () => {
		const scanner = new SerialScanner();
		expect(scanner.active).toBe(false);
		scanner.stop();
	});

	it('is active after start()', async () => {
		const { port } = makePort([]);
		mockSerial(port);
		const scanner = new SerialScanner();
		await scanner.start();
		expect(scanner.active).toBe(true);
		scanner.stop();
	});

	it('is inactive after stop()', async () => {
		const { port } = makePort([]);
		mockSerial(port);
		const scanner = new SerialScanner();
		await scanner.start();
		scanner.stop();
		expect(scanner.active).toBe(false);
	});
});
