/**
 * Persists the last N containers a user moved an item to.
 * Used by the scan page for one-tap recent-destination moves.
 */

const STORAGE_KEY = 'homorg:recent-move-targets';
const MAX_ENTRIES = 5;

export interface RecentContainer {
	id: string;
	name: string;
	container_path: string | null;
	parent_name: string | null;
}

function load(): RecentContainer[] {
	try {
		const raw = localStorage.getItem(STORAGE_KEY);
		return raw ? (JSON.parse(raw) as RecentContainer[]) : [];
	} catch {
		return [];
	}
}

function save(list: RecentContainer[]): void {
	try {
		localStorage.setItem(STORAGE_KEY, JSON.stringify(list));
	} catch { /* ignore quota errors */ }
}

/** Returns the current recent containers list. */
export function getRecentContainers(): RecentContainer[] {
	return load();
}

/** Prepends a container to the list, deduplicates by id, trims to MAX_ENTRIES. */
export function pushRecentContainer(container: RecentContainer): void {
	const list = load().filter((c) => c.id !== container.id);
	save([container, ...list].slice(0, MAX_ENTRIES));
}

/** Clears the list (e.g. on logout). */
export function clearRecentContainers(): void {
	try { localStorage.removeItem(STORAGE_KEY); } catch { /* ignore */ }
}
