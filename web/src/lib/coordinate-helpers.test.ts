import { describe, it, expect } from 'vitest';
import {
	parseLocationSchema,
	parseCoordinate,
	formatCoordinate,
	schemaTypeLabel
} from './coordinate-helpers';

describe('parseLocationSchema', () => {
	it('returns null for null/undefined', () => {
		expect(parseLocationSchema(null)).toBeNull();
		expect(parseLocationSchema(undefined)).toBeNull();
	});

	it('returns null for non-object', () => {
		expect(parseLocationSchema('string')).toBeNull();
		expect(parseLocationSchema(42)).toBeNull();
	});

	it('returns null for unknown type', () => {
		expect(parseLocationSchema({ type: 'unknown' })).toBeNull();
	});

	it('parses abstract schema', () => {
		expect(parseLocationSchema({ type: 'abstract' })).toEqual({ type: 'abstract', labels: undefined });
	});

	it('parses abstract schema with labels', () => {
		expect(parseLocationSchema({ type: 'abstract', labels: ['top', 'middle', 'bottom'] })).toEqual({
			type: 'abstract',
			labels: ['top', 'middle', 'bottom']
		});
	});

	it('parses grid schema', () => {
		expect(parseLocationSchema({ type: 'grid', rows: 3, columns: 5 })).toEqual({
			type: 'grid',
			rows: 3,
			columns: 5,
			row_labels: undefined,
			column_labels: undefined
		});
	});

	it('parses grid schema with labels', () => {
		expect(
			parseLocationSchema({ type: 'grid', rows: 2, columns: 2, row_labels: ['A', 'B'], column_labels: ['1', '2'] })
		).toEqual({
			type: 'grid',
			rows: 2,
			columns: 2,
			row_labels: ['A', 'B'],
			column_labels: ['1', '2']
		});
	});

	it('returns null for grid missing dimensions', () => {
		expect(parseLocationSchema({ type: 'grid', rows: 3 })).toBeNull();
		expect(parseLocationSchema({ type: 'grid', columns: 5 })).toBeNull();
	});

	it('parses geo schema', () => {
		expect(parseLocationSchema({ type: 'geo' })).toEqual({ type: 'geo' });
	});
});

describe('parseCoordinate', () => {
	it('returns null for null/undefined', () => {
		expect(parseCoordinate(null)).toBeNull();
		expect(parseCoordinate(undefined)).toBeNull();
	});

	it('returns null for non-object', () => {
		expect(parseCoordinate('string')).toBeNull();
	});

	it('returns null for unknown type', () => {
		expect(parseCoordinate({ type: 'xyz' })).toBeNull();
	});

	it('parses abstract coordinate', () => {
		expect(parseCoordinate({ type: 'abstract', value: 'top shelf' })).toEqual({
			type: 'abstract',
			value: 'top shelf'
		});
	});

	it('returns null for abstract without string value', () => {
		expect(parseCoordinate({ type: 'abstract', value: 123 })).toBeNull();
		expect(parseCoordinate({ type: 'abstract' })).toBeNull();
	});

	it('parses grid coordinate', () => {
		expect(parseCoordinate({ type: 'grid', row: 2, column: 4 })).toEqual({
			type: 'grid',
			row: 2,
			column: 4
		});
	});

	it('returns null for grid with non-number fields', () => {
		expect(parseCoordinate({ type: 'grid', row: '2', column: 4 })).toBeNull();
	});

	it('parses geo coordinate', () => {
		expect(parseCoordinate({ type: 'geo', latitude: 40.7128, longitude: -74.006 })).toEqual({
			type: 'geo',
			latitude: 40.7128,
			longitude: -74.006
		});
	});

	it('returns null for geo with missing fields', () => {
		expect(parseCoordinate({ type: 'geo', latitude: 40 })).toBeNull();
	});
});

describe('formatCoordinate', () => {
	it('returns empty string for null', () => {
		expect(formatCoordinate(null)).toBe('');
	});

	it('returns JSON for unknown coordinate shape', () => {
		expect(formatCoordinate({ foo: 'bar' })).toBe('{"foo":"bar"}');
	});

	it('formats abstract coordinate', () => {
		expect(formatCoordinate({ type: 'abstract', value: 'top shelf' })).toBe('top shelf');
	});

	it('formats grid coordinate with numeric labels', () => {
		expect(formatCoordinate({ type: 'grid', row: 0, column: 2 })).toBe('Row 1, Col 3');
	});

	it('formats grid coordinate with schema labels', () => {
		const schema = { type: 'grid', rows: 3, columns: 3, row_labels: ['A', 'B', 'C'], column_labels: ['I', 'II', 'III'] };
		expect(formatCoordinate({ type: 'grid', row: 1, column: 2 }, schema)).toBe('Row B, Col III');
	});

	it('formats geo coordinate', () => {
		expect(formatCoordinate({ type: 'geo', latitude: 40.712800, longitude: -74.006000 })).toBe(
			'40.712800, -74.006000'
		);
	});
	it('formats grid coordinate with schema labels out of bounds falls back to numeric', () => {
		const schema = { type: 'grid', rows: 2, columns: 2, row_labels: ['A', 'B'], column_labels: ['I', 'II'] };
		// Row 5 is beyond schema labels — should fall back to "6"
		expect(formatCoordinate({ type: 'grid', row: 5, column: 0 }, schema)).toBe('Row 6, Col I');
	});

	it('formats grid coordinate without schema is unaffected by null schema', () => {
		expect(formatCoordinate({ type: 'grid', row: 0, column: 0 }, null)).toBe('Row 1, Col 1');
	});

	it('formats grid coordinate with schema missing labels falls back to numeric', () => {
		const schema = { type: 'grid', rows: 3, columns: 3 };
		expect(formatCoordinate({ type: 'grid', row: 1, column: 2 }, schema)).toBe('Row 2, Col 3');
	});
});

describe('schemaTypeLabel', () => {
	it('returns "None" for null', () => {
		expect(schemaTypeLabel(null)).toBe('None');
	});

	it('returns "Custom" for unrecognized schema', () => {
		expect(schemaTypeLabel({ type: 'something_else' })).toBe('Custom');
	});

	it('returns "Labels" for abstract', () => {
		expect(schemaTypeLabel({ type: 'abstract' })).toBe('Labels');
	});

	it('returns grid dimensions', () => {
		expect(schemaTypeLabel({ type: 'grid', rows: 3, columns: 5 })).toBe('Grid (3\u00d75)');
	});

	it('returns "Geographic" for geo', () => {
		expect(schemaTypeLabel({ type: 'geo' })).toBe('Geographic');
	});
});
