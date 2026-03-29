declare module 'fake-indexeddb/lib/FDBFactory';

// Web Serial API — not yet in standard TS lib definitions.
// Minimal declarations for serial-scanner.ts usage.
interface SerialOptions {
	baudRate: number;
}

interface SerialPort {
	open(options: SerialOptions): Promise<void>;
	close(): Promise<void>;
	readonly readable: ReadableStream<Uint8Array> | null;
}
