/**
 * Barcode / code type definitions drawn from international standards.
 *
 * `STANDARD_CODE_TYPES` is the canonical list used to pre-populate type
 * selectors across the UI. Users may still enter custom types not on this
 * list. Auto-detection via `detectBarcodeType()` returns values from this
 * list where possible.
 */

export interface CodeTypeOption {
	/** Value stored in the database and displayed as the type label. */
	value: string;
	/** Short description shown as a tooltip / hint in selectors. */
	description: string;
}

export const STANDARD_CODE_TYPES: readonly CodeTypeOption[] = [
	// ── Retail / publishing ──────────────────────────────────────────────────
	{ value: 'ISBN',        description: 'Books & publications — 13-digit 978/979 prefix or legacy 10-digit' },
	{ value: 'UPC',         description: 'US retail products — 12 digits (UPC-A / UPC-E)' },
	{ value: 'EAN',         description: 'International retail — 13 digits (EAN-13)' },
	{ value: 'EAN-8',       description: 'Small retail products — 8 digits' },
	{ value: 'ISSN',        description: 'Periodicals & magazines — 8 digits' },
	// ── Supply chain ─────────────────────────────────────────────────────────
	{ value: 'GTIN',        description: 'Global Trade Item Number — 14 digits, superset of UPC/EAN' },
	{ value: 'ITF',         description: 'Interleaved 2 of 5 — shipping cartons & logistics' },
	// ── General-purpose linear ────────────────────────────────────────────────
	{ value: 'Code 128',    description: 'High-density alphanumeric linear barcode' },
	{ value: 'Code 39',     description: 'Alphanumeric linear barcode (automotive, defence)' },
	{ value: 'Codabar',     description: 'Numeric linear barcode (libraries, blood banks)' },
	// ── 2D ────────────────────────────────────────────────────────────────────
	{ value: 'QR',          description: 'QR Code — 2D matrix, very common' },
	{ value: 'Data Matrix', description: '2D matrix code — small labels, electronics' },
	{ value: 'PDF417',      description: '2D stacked barcode — ID cards, boarding passes' },
	{ value: 'Aztec',       description: 'Aztec code — transport tickets, boarding passes' },
] as const;

/** O(1) membership check: is this value a known standard type? */
export const STANDARD_CODE_TYPE_VALUES: ReadonlySet<string> =
	new Set(STANDARD_CODE_TYPES.map((t) => t.value));

// ─── Auto-detection ──────────────────────────────────────────────────────────

/**
 * Detect the code type from a barcode value and/or the format string returned
 * by the BarcodeDetector API. Returns a value from STANDARD_CODE_TYPES where
 * possible, or '' when the type cannot be determined.
 */
export function detectBarcodeType(value: string, format?: string): string {
	// Trust BarcodeDetector format first — it's authoritative when available.
	if (format) {
		switch (format) {
			case 'upc_a':
			case 'upc_e':
				return 'UPC';
			case 'ean_13':
				return /^97[89]/.test(value) ? 'ISBN' : 'EAN';
			case 'ean_8':
				return 'EAN-8';
			case 'isbn':
				return 'ISBN';
			case 'qr_code':
				return 'QR';
			case 'code_128':
				return 'Code 128';
			case 'code_39':
				return 'Code 39';
			case 'codabar':
				return 'Codabar';
			case 'data_matrix':
				return 'Data Matrix';
			case 'pdf417':
				return 'PDF417';
			case 'aztec':
				return 'Aztec';
			case 'itf':
				return 'ITF';
		}
	}

	// Heuristic fallback: infer from value structure.
	const digits = value.replace(/[-\s]/g, '');

	if (/^\d{14}$/.test(digits)) return 'GTIN';
	if (/^\d{13}$/.test(digits)) return /^97[89]/.test(digits) ? 'ISBN' : 'EAN';
	if (/^\d{12}$/.test(digits)) return 'UPC';
	if (/^\d{9}[\dX]$/i.test(digits) && isISBN10(digits)) return 'ISBN';
	if (/^\d{8}$/.test(digits)) return 'EAN-8';

	return '';
}

/** Validate ISBN-10 check digit. */
function isISBN10(digits: string): boolean {
	let sum = 0;
	for (let i = 0; i < 9; i++) sum += (10 - i) * Number(digits[i]);
	const last = digits[9].toUpperCase();
	sum += last === 'X' ? 10 : Number(last);
	return sum % 11 === 0;
}
