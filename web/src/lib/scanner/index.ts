/**
 * Svelte store binding for the active scanner.
 * Manages the active scanner instance and exposes reactive state.
 */

import { writable, derived } from 'svelte/store';
import { HidScanner } from './hid-scanner.js';
import type { ScanEvent } from './scanner.js';

export type ScannerSource = 'hid' | 'serial' | 'camera' | 'none';
export type ScannerStatus = 'idle' | 'active' | 'error';

interface ScannerState {
	source: ScannerSource;
	status: ScannerStatus;
	errorMessage: string | null;
}

const initialState: ScannerState = {
	source: 'none',
	status: 'idle',
	errorMessage: null
};

export const scannerState = writable<ScannerState>(initialState);
export const isScanning = derived(scannerState, (s) => s.status === 'active');

type ScanHandler = (event: ScanEvent) => void;

let activeScanner: HidScanner | null = null;
let unsub: (() => void) | null = null;
const handlers: Set<ScanHandler> = new Set();

/** Start the HID scanner (always available, no permissions needed). */
export async function startHidScanner() {
	stopScanner();
	const scanner = new HidScanner();
	unsub = scanner.onScan((e) => {
		handlers.forEach((h) => h(e));
	});
	await scanner.start();
	activeScanner = scanner;
	scannerState.set({ source: 'hid', status: 'active', errorMessage: null });
}

/** Start the Web Serial scanner. Requires a user gesture. */
export async function startSerialScanner() {
	const { SerialScanner } = await import('./serial-scanner.js');
	stopScanner();
	const scanner = new SerialScanner();
	unsub = scanner.onScan((e) => {
		handlers.forEach((h) => h(e));
	});
	try {
		await scanner.start();
		scannerState.set({ source: 'serial', status: 'active', errorMessage: null });
	} catch (err) {
		scannerState.set({
			source: 'none',
			status: 'error',
			errorMessage: err instanceof Error ? err.message : 'Serial connection failed'
		});
	}
}

/** Start the camera scanner. Requires getUserMedia permission. */
export async function startCameraScanner(): Promise<HTMLVideoElement | null> {
	const { CameraScanner } = await import('./camera-scanner.js');
	stopScanner();
	const scanner = new CameraScanner();
	unsub = scanner.onScan((e) => {
		handlers.forEach((h) => h(e));
	});
	try {
		await scanner.start();
		scannerState.set({ source: 'camera', status: 'active', errorMessage: null });
		return scanner.videoElement;
	} catch (err) {
		scannerState.set({
			source: 'none',
			status: 'error',
			errorMessage: err instanceof Error ? err.message : 'Camera access failed'
		});
		return null;
	}
}

/** Stop whichever scanner is currently running. */
export function stopScanner() {
	unsub?.();
	unsub = null;
	activeScanner?.stop();
	activeScanner = null;
	scannerState.set(initialState);
}

/** Subscribe to scan events from any active scanner. */
export function onScan(handler: ScanHandler): () => void {
	handlers.add(handler);
	return () => handlers.delete(handler);
}
