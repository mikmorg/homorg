/**
 * Detect the human-readable type of a barcode from its value and/or the
 * format string returned by the BarcodeDetector API.
 *
 * Returns one of the type strings used by this app's external_codes convention:
 *   'UPC', 'EAN', 'ISBN', 'QR', 'Code 128', 'Code 39', 'Data Matrix', 'PDF417'
 * Returns '' when the type cannot be determined.
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
				return 'EAN';
			case 'isbn':
				return 'ISBN';
			case 'qr_code':
				return 'QR';
			case 'code_128':
				return 'Code 128';
			case 'code_39':
				return 'Code 39';
			case 'data_matrix':
				return 'Data Matrix';
			case 'pdf417':
				return 'PDF417';
		}
	}

	// Heuristic fallback: infer from value structure.
	const digits = value.replace(/[-\s]/g, '');

	if (/^\d{13}$/.test(digits)) {
		return /^97[89]/.test(digits) ? 'ISBN' : 'EAN';
	}
	if (/^\d{12}$/.test(digits)) return 'UPC';
	if (/^\d{10}$/.test(digits) && isISBN10(digits)) return 'ISBN';
	if (/^\d{8}$/.test(digits)) return 'EAN';

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
