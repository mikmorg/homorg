/**
 * Stocker session store — tracks the active scan session state reactively.
 */

import { writable, derived } from 'svelte/store';
import type { ScanSession, Item } from '$api/types.js';

export interface SessionContext {
	containerId: string | null;
	containerName: string | null;
	containerBarcode: string | null;
}

export interface StockerState {
	session: ScanSession | null;
	context: SessionContext;
	/** Items recently moved/created in this session, newest first. */
	recentItems: Item[];
	/** Batched events waiting to be flushed to the server. */
	pendingCount: number;
	lastSyncAt: number | null;
	error: string | null;
}

const initial: StockerState = {
	session: null,
	context: { containerId: null, containerName: null, containerBarcode: null },
	recentItems: [],
	pendingCount: 0,
	lastSyncAt: null,
	error: null
};

export const stockerStore = writable<StockerState>(initial);

export const hasActiveSession = derived(stockerStore, (s) => s.session !== null);
export const activeSession = derived(stockerStore, (s) => s.session);
export const activeContext = derived(stockerStore, (s) => s.context);
export const recentItems = derived(stockerStore, (s) => s.recentItems);

export function setSession(session: ScanSession) {
	stockerStore.update((s) => ({ ...s, session, error: null }));
}

export function clearSession() {
	stockerStore.set(initial);
}

export function setContext(ctx: SessionContext) {
	stockerStore.update((s) => ({ ...s, context: ctx }));
}

export function addRecentItem(item: Item) {
	stockerStore.update((s) => ({
		...s,
		recentItems: [item, ...s.recentItems].slice(0, 50)
	}));
}

export function setError(error: string | null) {
	stockerStore.update((s) => ({ ...s, error }));
}

export function setPendingCount(n: number) {
	stockerStore.update((s) => ({ ...s, pendingCount: n }));
}

export function markSynced() {
	stockerStore.update((s) => ({ ...s, pendingCount: 0, lastSyncAt: Date.now() }));
}
