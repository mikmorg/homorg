import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { HidScanner } from './hid-scanner.js';

// Helper: fire a sequence of keydown events on window with controlled timing.
// `time` is the value performance.now() returns for that key.
function fireKey(key: string, time: number, target?: EventTarget) {
	const event = new KeyboardEvent('keydown', { key, bubbles: true });
	if (target) {
		Object.defineProperty(event, 'target', { value: target, configurable: true });
	}
	vi.spyOn(performance, 'now').mockReturnValue(time);
	window.dispatchEvent(event);
}

// Fire a rapid burst of keys simulating a HID scanner (5ms between chars).
function fireBurst(barcode: string, startTime = 100) {
	let t = startTime;
	for (const ch of barcode) {
		fireKey(ch, t);
		t += 5; // scanner-speed: well under 50ms threshold
	}
	fireKey('Enter', t);
}

describe('HidScanner', () => {
	let scanner: HidScanner;
	let scans: { barcode: string; source: string }[];
	let unsubscribe: () => void;

	beforeEach(async () => {
		scanner = new HidScanner();
		scans = [];
		await scanner.start();
		unsubscribe = scanner.onScan((e) => scans.push(e));
		vi.spyOn(performance, 'now').mockReturnValue(0);
	});

	afterEach(() => {
		unsubscribe();
		scanner.stop();
		vi.restoreAllMocks();
	});

	// ── Normal scan ─────────────────────────────────────────────────────────

	it('emits barcode on rapid burst followed by Enter', () => {
		fireBurst('HOM-000042');
		expect(scans).toHaveLength(1);
		expect(scans[0]).toEqual({ barcode: 'HOM-000042', source: 'hid' });
	});

	it('trims whitespace from emitted barcode', () => {
		// Some scanners append a trailing space before Enter
		fireBurst('HOM-000042 ');
		expect(scans[0].barcode).toBe('HOM-000042');
	});

	it('emits multiple consecutive scans', () => {
		fireBurst('HOM-000001', 100);
		fireBurst('HOM-000002', 300);
		expect(scans).toHaveLength(2);
		expect(scans[0].barcode).toBe('HOM-000001');
		expect(scans[1].barcode).toBe('HOM-000002');
	});

	// ── Minimum length guard ─────────────────────────────────────────────────

	it('does not emit when barcode is shorter than 4 chars', () => {
		fireBurst('ABC'); // 3 chars
		expect(scans).toHaveLength(0);
	});

	it('emits at the minimum of 4 chars', () => {
		fireBurst('ABCD');
		expect(scans).toHaveLength(1);
		expect(scans[0].barcode).toBe('ABCD');
	});

	// ── Inter-character gap detection ────────────────────────────────────────

	it('clears buffer when inter-char gap exceeds threshold', () => {
		// Type slowly (100ms gap) — should not emit
		fireKey('H', 100);
		fireKey('O', 200); // 100ms gap > 50ms → clears buffer, then appends 'O'
		fireKey('M', 205);
		fireKey('Enter', 210);
		// Buffer after gap: "OM" → too short (2 chars) → no emit
		expect(scans).toHaveLength(0);
	});

	it('emits correctly when gap occurs but enough chars follow quickly', () => {
		// Slow start, then fast burst
		fireKey('H', 100);
		fireKey('O', 200); // gap clears. buffer now = 'O'
		fireKey('M', 205);
		fireKey('-', 210);
		fireKey('1', 215);
		fireKey('2', 220);
		fireKey('3', 225);
		fireKey('Enter', 230);
		// buffer after gap: "OM-123" → 6 chars → emits
		expect(scans).toHaveLength(1);
		expect(scans[0].barcode).toBe('OM-123');
	});

	// ── Non-printable key filtering ──────────────────────────────────────────

	it('ignores non-printable keys (length > 1)', () => {
		// Shift, Control, Alt etc. should not be appended
		fireKey('Shift', 100);
		fireKey('H', 105);
		fireKey('O', 110);
		fireKey('M', 115);
		fireKey('-', 120);
		fireKey('0', 125);
		fireKey('1', 130);
		fireKey('Enter', 135);
		expect(scans[0].barcode).toBe('HOM-01');
	});

	// ── Input field suppression ───────────────────────────────────────────────

	it('ignores keydown events from INPUT elements by default', () => {
		const input = document.createElement('input');
		document.body.appendChild(input);
		try {
			for (let i = 0; i < 6; i++) {
				const e = new KeyboardEvent('keydown', { key: String(i), bubbles: true });
				Object.defineProperty(e, 'target', { value: input, configurable: true });
				vi.spyOn(performance, 'now').mockReturnValue(100 + i * 5);
				window.dispatchEvent(e);
			}
			const e = new KeyboardEvent('keydown', { key: 'Enter', bubbles: true });
			Object.defineProperty(e, 'target', { value: input, configurable: true });
			window.dispatchEvent(e);
			expect(scans).toHaveLength(0);
		} finally {
			document.body.removeChild(input);
		}
	});

	it('processes keydown from INPUT elements that have data-scanner-target', () => {
		const input = document.createElement('input');
		input.dataset.scannerTarget = 'true';
		document.body.appendChild(input);
		try {
			'HOM-01'.split('').forEach((ch, i) => {
				const e = new KeyboardEvent('keydown', { key: ch, bubbles: true });
				Object.defineProperty(e, 'target', { value: input, configurable: true });
				vi.spyOn(performance, 'now').mockReturnValue(100 + i * 5);
				window.dispatchEvent(e);
			});
			const e = new KeyboardEvent('keydown', { key: 'Enter', bubbles: true });
			Object.defineProperty(e, 'target', { value: input, configurable: true });
			window.dispatchEvent(e);
			expect(scans).toHaveLength(1);
			expect(scans[0].barcode).toBe('HOM-01');
		} finally {
			document.body.removeChild(input);
		}
	});

	// ── Lifecycle ─────────────────────────────────────────────────────────────

	it('marks scanner as active after start()', () => {
		expect(scanner.active).toBe(true);
	});

	it('marks scanner as inactive and stops emitting after stop()', () => {
		scanner.stop();
		expect(scanner.active).toBe(false);
		fireBurst('HOM-000042', 500);
		expect(scans).toHaveLength(0);
	});
});
