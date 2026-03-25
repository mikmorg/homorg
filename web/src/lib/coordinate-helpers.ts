import type { KnownLocationSchema, KnownCoordinate } from '$api/types.js';

export function parseLocationSchema(raw: unknown): KnownLocationSchema | null {
	if (!raw || typeof raw !== 'object') return null;
	const obj = raw as Record<string, unknown>;
	switch (obj.type) {
		case 'abstract':
			return { type: 'abstract', labels: Array.isArray(obj.labels) ? obj.labels : undefined };
		case 'grid':
			if (typeof obj.rows === 'number' && typeof obj.columns === 'number') {
				return {
					type: 'grid',
					rows: obj.rows,
					columns: obj.columns,
					row_labels: Array.isArray(obj.row_labels) ? obj.row_labels : undefined,
					column_labels: Array.isArray(obj.column_labels) ? obj.column_labels : undefined
				};
			}
			return null;
		case 'geo':
			return { type: 'geo' };
		default:
			return null;
	}
}

export function parseCoordinate(raw: unknown): KnownCoordinate | null {
	if (!raw || typeof raw !== 'object') return null;
	const obj = raw as Record<string, unknown>;
	switch (obj.type) {
		case 'abstract':
			return typeof obj.value === 'string' ? { type: 'abstract', value: obj.value } : null;
		case 'grid':
			return typeof obj.row === 'number' && typeof obj.column === 'number'
				? { type: 'grid', row: obj.row, column: obj.column }
				: null;
		case 'geo':
			return typeof obj.latitude === 'number' && typeof obj.longitude === 'number'
				? { type: 'geo', latitude: obj.latitude, longitude: obj.longitude }
				: null;
		default:
			return null;
	}
}

export function formatCoordinate(raw: unknown, schema?: unknown): string {
	const coord = parseCoordinate(raw);
	if (!coord) return raw ? JSON.stringify(raw) : '';
	switch (coord.type) {
		case 'abstract':
			return coord.value;
		case 'grid': {
			const s = parseLocationSchema(schema);
			const rowLabel =
				s?.type === 'grid' && s.row_labels?.[coord.row]
					? s.row_labels[coord.row]
					: String(coord.row + 1);
			const colLabel =
				s?.type === 'grid' && s.column_labels?.[coord.column]
					? s.column_labels[coord.column]
					: String(coord.column + 1);
			return `Row ${rowLabel}, Col ${colLabel}`;
		}
		case 'geo':
			return `${coord.latitude.toFixed(6)}, ${coord.longitude.toFixed(6)}`;
	}
}

export function computeLabelRenames(
	originalLabels: string[],
	newLabels: string[]
): Record<string, string> {
	const removed = originalLabels.filter((l) => !newLabels.includes(l));
	const added = newLabels.filter((l) => !originalLabels.includes(l));
	const renames: Record<string, string> = {};
	for (let i = 0; i < Math.min(removed.length, added.length); i++) {
		renames[removed[i]] = added[i];
	}
	return renames;
}

export function schemaTypeLabel(raw: unknown): string {
	const s = parseLocationSchema(raw);
	if (!s) return raw ? 'Custom' : 'None';
	switch (s.type) {
		case 'abstract':
			return 'Labels';
		case 'grid':
			return `Grid (${s.rows}\u00d7${s.columns})`;
		case 'geo':
			return 'Geographic';
	}
}
