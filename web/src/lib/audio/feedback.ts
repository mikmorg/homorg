/**
 * Synthesized audio feedback for the stocker scanner interface.
 * Uses the Web Audio API to produce distinct tones without network overhead.
 * AudioContext requires a user gesture to resume — call init() on first interaction.
 */

let ctx: AudioContext | null = null;

function getCtx(): AudioContext {
	if (!ctx) {
		ctx = new (window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext)();
	}
	if (ctx.state === 'suspended') {
		ctx.resume().catch(() => {});
	}
	return ctx;
}

function tone(
	frequency: number,
	durationMs: number,
	type: OscillatorType = 'sine',
	gainValue = 0.3,
	delayMs = 0
): void {
	try {
		const c = getCtx();
		const osc = c.createOscillator();
		const gain = c.createGain();
		osc.connect(gain);
		gain.connect(c.destination);

		osc.type = type;
		osc.frequency.value = frequency;
		gain.gain.setValueAtTime(0, c.currentTime + delayMs / 1000);
		gain.gain.linearRampToValueAtTime(gainValue, c.currentTime + delayMs / 1000 + 0.005);
		gain.gain.exponentialRampToValueAtTime(0.001, c.currentTime + delayMs / 1000 + durationMs / 1000);

		osc.start(c.currentTime + delayMs / 1000);
		osc.stop(c.currentTime + delayMs / 1000 + durationMs / 1000 + 0.01);
	} catch {
		// AudioContext may not be available in all environments
	}
}

function vibrate(pattern: number | number[]) {
	if (typeof navigator !== 'undefined' && navigator.vibrate) {
		navigator.vibrate(pattern);
	}
}

/**
 * Initialize the AudioContext on first user gesture.
 * Call this from a click/touchstart handler.
 */
export function init() {
	getCtx();
}

/**
 * Successful scan — item moved to container.
 * Sharp high tick, single short vibration.
 */
export function scanSuccess() {
	tone(880, 80, 'sine', 0.25);
	vibrate(40);
}

/**
 * Container context set — active container changed.
 * Rising two-tone sweep.
 */
export function contextSet() {
	tone(440, 100, 'sine', 0.25);
	tone(660, 120, 'sine', 0.2, 80);
	vibrate([30, 20, 30]);
}

/**
 * Unknown system barcode — new item needs to be created.
 * Two-tone double chirp, double vibration.
 */
export function newItem() {
	tone(660, 90, 'sine', 0.3);
	tone(880, 90, 'sine', 0.25, 100);
	vibrate([60, 40, 120]);
}

/**
 * Scan error — item not found, wrong context, API failure, etc.
 * Low dissonant buzz.
 */
export function scanError() {
	tone(220, 200, 'square', 0.2);
	vibrate([80, 30, 80]);
}

/**
 * Warning — ambiguous scan (external code matches multiple items).
 */
export function scanWarning() {
	tone(440, 120, 'triangle', 0.2);
	tone(330, 120, 'triangle', 0.15, 100);
	vibrate([50, 30, 50]);
}

/**
 * Batch submitted successfully.
 */
export function batchSynced() {
	tone(660, 60, 'sine', 0.15);
	tone(880, 60, 'sine', 0.1, 70);
	tone(1100, 80, 'sine', 0.1, 140);
}
