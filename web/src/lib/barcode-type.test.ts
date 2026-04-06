import { describe, it, expect } from 'vitest';
import { detectBarcodeType, STANDARD_CODE_TYPES, STANDARD_CODE_TYPE_VALUES } from './barcode-type.js';

// ── STANDARD_CODE_TYPES catalogue ────────────────────────────────────────────

describe('STANDARD_CODE_TYPES', () => {
	it('contains 14 entries', () => {
		expect(STANDARD_CODE_TYPES).toHaveLength(14);
	});

	it('STANDARD_CODE_TYPE_VALUES set matches the array values', () => {
		for (const t of STANDARD_CODE_TYPES) {
			expect(STANDARD_CODE_TYPE_VALUES.has(t.value)).toBe(true);
		}
	});
});

// ── detectBarcodeType — format parameter (BarcodeDetector API) ───────────────

describe('detectBarcodeType — format parameter', () => {
	it('upc_a → UPC', () => expect(detectBarcodeType('012345678905', 'upc_a')).toBe('UPC'));
	it('upc_e → UPC', () => expect(detectBarcodeType('01234565', 'upc_e')).toBe('UPC'));

	it('ean_13 without 978/979 prefix → EAN', () => {
		expect(detectBarcodeType('5901234123457', 'ean_13')).toBe('EAN');
	});
	it('ean_13 with 978 prefix → ISBN', () => {
		expect(detectBarcodeType('9780306406157', 'ean_13')).toBe('ISBN');
	});
	it('ean_13 with 979 prefix → ISBN', () => {
		expect(detectBarcodeType('9791032317990', 'ean_13')).toBe('ISBN');
	});

	it('ean_8 → EAN-8', () => expect(detectBarcodeType('73513537', 'ean_8')).toBe('EAN-8'));
	it('isbn → ISBN', () => expect(detectBarcodeType('9780306406157', 'isbn')).toBe('ISBN'));
	it('qr_code → QR', () => expect(detectBarcodeType('https://example.com', 'qr_code')).toBe('QR'));
	it('code_128 → Code 128', () => expect(detectBarcodeType('ABC-123', 'code_128')).toBe('Code 128'));
	it('code_39 → Code 39', () => expect(detectBarcodeType('HELLO', 'code_39')).toBe('Code 39'));
	it('codabar → Codabar', () => expect(detectBarcodeType('A12345B', 'codabar')).toBe('Codabar'));
	it('data_matrix → Data Matrix', () => expect(detectBarcodeType('abc', 'data_matrix')).toBe('Data Matrix'));
	it('pdf417 → PDF417', () => expect(detectBarcodeType('abc', 'pdf417')).toBe('PDF417'));
	it('aztec → Aztec', () => expect(detectBarcodeType('abc', 'aztec')).toBe('Aztec'));
	it('itf → ITF', () => expect(detectBarcodeType('01234567890128', 'itf')).toBe('ITF'));

	it('unknown format string → falls through to heuristic', () => {
		// 12 digits → UPC heuristic
		expect(detectBarcodeType('012345678905', 'unknown_format')).toBe('UPC');
	});
});

// ── detectBarcodeType — heuristic (no format) ─────────────────────────────────

describe('detectBarcodeType — heuristic fallback', () => {
	it('14 digits → GTIN', () => {
		expect(detectBarcodeType('12345678901234')).toBe('GTIN');
	});

	it('13 digits with 978 prefix → ISBN', () => {
		expect(detectBarcodeType('9780306406157')).toBe('ISBN');
	});
	it('13 digits with 979 prefix → ISBN', () => {
		expect(detectBarcodeType('9791032317990')).toBe('ISBN');
	});
	it('13 digits without 97x prefix → EAN', () => {
		expect(detectBarcodeType('5901234123457')).toBe('EAN');
	});

	it('12 digits → UPC', () => {
		expect(detectBarcodeType('012345678905')).toBe('UPC');
	});

	it('10 digit valid ISBN-10 → ISBN', () => {
		// 0306406152 is a valid ISBN-10 (sum mod 11 = 0)
		expect(detectBarcodeType('0306406152')).toBe('ISBN');
	});
	it('10 digits with invalid ISBN-10 checksum → empty string', () => {
		// Change last digit so checksum fails
		expect(detectBarcodeType('0306406153')).toBe('');
	});

	it('8 digits → EAN-8', () => {
		expect(detectBarcodeType('73513537')).toBe('EAN-8');
	});

	it('format takes precedence over heuristic', () => {
		// 8 digits would be EAN-8 by heuristic, but format says UPC-E
		expect(detectBarcodeType('01234565', 'upc_e')).toBe('UPC');
	});
});

// ── detectBarcodeType — hyphen/space stripping ────────────────────────────────

describe('detectBarcodeType — hyphens and spaces stripped', () => {
	it('14 digits with hyphens → GTIN', () => {
		expect(detectBarcodeType('1234-5678-9012-34')).toBe('GTIN');
	});
	it('12 digits with spaces → UPC', () => {
		expect(detectBarcodeType('0123 4567 8905')).toBe('UPC');
	});
	it('13 digits with hyphens → EAN or ISBN', () => {
		// 978-0-306-40615-7
		expect(detectBarcodeType('978-0-306-40615-7')).toBe('ISBN');
	});
});

// ── detectBarcodeType — unrecognised inputs ───────────────────────────────────

describe('detectBarcodeType — unrecognised inputs', () => {
	it('returns empty string for 7-digit number', () => {
		expect(detectBarcodeType('1234567')).toBe('');
	});
	it('returns empty string for alphanumeric string', () => {
		expect(detectBarcodeType('HELLO')).toBe('');
	});
	it('returns empty string for empty string', () => {
		expect(detectBarcodeType('')).toBe('');
	});
	it('returns empty string for 11 digits', () => {
		expect(detectBarcodeType('12345678901')).toBe('');
	});
	it('returns empty string for 9 digits', () => {
		expect(detectBarcodeType('123456789')).toBe('');
	});
});

// ── ISBN-10 checksum (exercised indirectly via detectBarcodeType) ─────────────

describe('ISBN-10 checksum', () => {
	// Valid ISBN-10 examples (check digit = X handled)
	it('accepts ISBN-10 with X check digit', () => {
		// 020161622X is a valid ISBN-10 (sum=110, 110%11=0)
		expect(detectBarcodeType('020161622X')).toBe('ISBN');
	});
	it('rejects ISBN-10 where X is not in last position', () => {
		// X in non-last position: treat as non-numeric, not a valid 10-digit number
		expect(detectBarcodeType('X306406152')).toBe('');
	});
	it('rejects 10-digit value with invalid checksum', () => {
		// 0000000001 has sum=1, 1%11≠0 → invalid checksum
		expect(detectBarcodeType('0000000001')).toBe('');
	});
});
