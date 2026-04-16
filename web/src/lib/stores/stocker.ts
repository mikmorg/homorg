/**
 * Stocker session store — tracks the active scan session state reactively.
 */

import { writable, derived } from 'svelte/store';
import type { ScanSession, Item } from '$api/types.js';

export interface SessionContext {
	containerId: string | null;
	containerName: string | null;
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
	/** ID of the most recently created/scanned item (camera target). */
	activeItemId: string | null;
}

const initial: StockerState = {
	session: null,
	context: { containerId: null, containerName: null },
	recentItems: [],
	pendingCount: 0,
	lastSyncAt: null,
	error: null,
	activeItemId: null
};

export const stockerStore = writable<StockerState>(initial);

export const hasActiveSession = derived(stockerStore, (s) => s.session !== null);
export const activeSession = derived(stockerStore, (s) => s.session);
export const activeContext = derived(stockerStore, (s) => s.context);
export const recentItems = derived(stockerStore, (s) => s.recentItems);
export const activeItemId = derived(stockerStore, (s) => s.activeItemId);

export function setSession(session: ScanSession) {
	stockerStore.update((s) => {
		// Session transition: different id (or first session) — wipe session-
		// scoped state so a stale context/activeItemId/recentItems from a prior
		// closed session can't leak into the new one.
		if (s.session?.id !== session.id) {
			return {
				...initial,
				session,
				activeItemId: session.active_item_id ?? null
			};
		}
		// Same session — a stats refresh after flushBatch etc. Preserve local
		// optimistic activeItemId when the server hasn't caught up yet.
		return {
			...s,
			session,
			error: null,
			activeItemId: session.active_item_id ?? s.activeItemId
		};
	});
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
		recentItems: [item, ...s.recentItems].slice(0, 50),
		activeItemId: item.id
	}));
}

export function setActiveItemId(itemId: string | null) {
	stockerStore.update((s) => ({ ...s, activeItemId: itemId }));
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
