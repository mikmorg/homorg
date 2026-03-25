/**
 * Offline mutation queue backed by IndexedDB.
 * Mutations are enqueued when the API is unreachable and retried when online.
 *
 * Each entry stores enough information to replay the request:
 *   { url, method, body, timestamp, attempts }
 *
 * Callers can subscribe to queue length changes to show a badge / indicator.
 */

import { openDB, type DBSchema, type IDBPDatabase } from 'idb';
import { writable, get } from 'svelte/store';

export interface PendingMutation {
	id?: number;
	url: string;
	method: string;
	headers: Record<string, string>;
	body: string | null;
	timestamp: number;
	attempts: number;
}

interface QueueSchema extends DBSchema {
	pendingMutations: {
		key: number;
		value: PendingMutation;
		indexes: { 'by-timestamp': number };
	};
}

const DB_NAME = 'homorg-offline';
const DB_VERSION = 1;
const MAX_ATTEMPTS = 20;
let db: IDBPDatabase<QueueSchema> | null = null;

async function getDb(): Promise<IDBPDatabase<QueueSchema>> {
	if (!db) {
		db = await openDB<QueueSchema>(DB_NAME, DB_VERSION, {
			upgrade(database) {
				const store = database.createObjectStore('pendingMutations', {
					keyPath: 'id',
					autoIncrement: true
				});
				store.createIndex('by-timestamp', 'timestamp');
			}
		});
	}
	return db;
}

/** Reactive queue length — 0 means in-sync. */
export const pendingCount = writable(0);

async function refreshCount() {
	try {
		const d = await getDb();
		const count = await d.count('pendingMutations');
		pendingCount.set(count);
	} catch {
		// IndexedDB unavailable (SSR or private browsing)
	}
}

/** Add a mutation to the queue. Call when a request fails while offline. */
export async function enqueue(mutation: Omit<PendingMutation, 'id' | 'attempts' | 'timestamp'>) {
	try {
		const d = await getDb();
		await d.add('pendingMutations', {
			...mutation,
			timestamp: Date.now(),
			attempts: 0
		});
		await refreshCount();
	} catch (e) {
		console.error('[offline-queue] Failed to enqueue mutation', e);
	}
}

/**
 * Attempt to replay all queued mutations in FIFO order.
 * Each mutation is retried with the current access token.
 * On permanent failure (4xx) the entry is discarded.
 * On transient failure (5xx / network error) the attempt counter increments.
 */
export async function sync(getToken: () => string | null): Promise<void> {
	if (!navigator.onLine) return;

	let database: IDBPDatabase<QueueSchema>;
	try {
		database = await getDb();
	} catch {
		return;
	}

	const all = await database.getAllFromIndex('pendingMutations', 'by-timestamp');

	for (const mutation of all) {
		// OQ-1: Drop mutations that have exceeded the retry limit.
		if (mutation.attempts >= MAX_ATTEMPTS) {
			console.warn('[offline-queue] Dropping mutation after max attempts', mutation.url);
			await database.delete('pendingMutations', mutation.id!);
			continue;
		}

		const token = getToken();
		const headers: Record<string, string> = {
			...mutation.headers,
			...(token ? { Authorization: `Bearer ${token}` } : {})
		};

		try {
			const res = await fetch(mutation.url, {
				method: mutation.method,
				headers,
				body: mutation.body ?? undefined
			});

			// OQ-2: 401 means the token expired — treat as transient so the
			// mutation is retried on the next sync with a fresh token.
			if (res.ok || (res.status >= 400 && res.status < 500 && res.status !== 401)) {
				// Success or permanent client error — remove from queue
				await database.delete('pendingMutations', mutation.id!);
			} else {
				// Transient server error — increment attempts
				await database.put('pendingMutations', {
					...mutation,
					attempts: mutation.attempts + 1
				});
			}
		} catch {
			// Network failure — increment attempts
			await database.put('pendingMutations', {
				...mutation,
				attempts: mutation.attempts + 1
			});
		}
	}

	await refreshCount();
}

/** Remove all queued mutations (e.g. after logout). */
export async function clear() {
	try {
		const d = await getDb();
		await d.clear('pendingMutations');
		pendingCount.set(0);
	} catch {
		// ignore
	}
}

/** Register online/offline listeners to auto-sync when connectivity returns. */
export function registerSyncListeners(getToken: () => string | null) {
	if (typeof window === 'undefined') return;

	window.addEventListener('online', () => {
		sync(getToken).catch(console.error);
	});

	// Initial count load
	refreshCount().catch(() => {});

	// Register Background Sync if supported
	if ('serviceWorker' in navigator && 'SyncManager' in window) {
		navigator.serviceWorker.ready
			.then((reg) => {
				// @ts-expect-error SyncManager not yet in all TS lib defs
				return reg.sync.register('homorg-offline-sync');
			})
			.catch(() => {});
	}
}
