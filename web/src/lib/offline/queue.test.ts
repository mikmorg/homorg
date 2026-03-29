// fake-indexeddb/auto registers all IDB globals (IDBRequest, IDBDatabase, etc.)
// that idb's wrap() function needs for instanceof checks.
import 'fake-indexeddb/auto';
import FDBFactory from 'fake-indexeddb/lib/FDBFactory';

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { get } from 'svelte/store';

// Dynamically imported per test so module-level state (db, isSyncing) resets.
type QueueMod = typeof import('./queue.js');
let mod: QueueMod;

beforeEach(async () => {
	// Fresh isolated IndexedDB for each test — new FDBFactory = empty in-memory store.
	vi.stubGlobal('indexedDB', new FDBFactory());
	// Reset cached module state (db = null, isSyncing = false).
	vi.resetModules();
	mod = await import('./queue.js');
	Object.defineProperty(navigator, 'onLine', { value: true, configurable: true });
});

afterEach(() => {
	vi.restoreAllMocks();
});

// Helper: minimal mutation payload.
function mutation(url = '/api/v1/items', method = 'POST', body: string | null = '{"name":"x"}') {
	return { url, method, headers: { 'Content-Type': 'application/json' }, body };
}

function mockFetch(...responses: Partial<Response>[]) {
	const fn = vi.fn();
	for (const r of responses) fn.mockResolvedValueOnce(r);
	vi.stubGlobal('fetch', fn);
	return fn;
}

const okResponse = (): Partial<Response> => ({ ok: true, status: 200 } as Partial<Response>);
const serverError = (): Partial<Response> => ({ ok: false, status: 500 } as Partial<Response>);
const clientError = (status = 404): Partial<Response> => ({ ok: false, status } as Partial<Response>);

// ── enqueue ────────────────────────────────────────────────────────────────

describe('enqueue', () => {
	it('adds a mutation and increments pendingCount', async () => {
		expect(get(mod.pendingCount)).toBe(0);
		await mod.enqueue(mutation());
		expect(get(mod.pendingCount)).toBe(1);
	});

	it('accumulates multiple mutations', async () => {
		await mod.enqueue(mutation('/api/v1/items', 'POST'));
		await mod.enqueue(mutation('/api/v1/items/1', 'PUT'));
		await mod.enqueue(mutation('/api/v1/items/2', 'DELETE', null));
		expect(get(mod.pendingCount)).toBe(3);
	});
});

// ── sync — success paths ───────────────────────────────────────────────────

describe('sync — success', () => {
	it('dequeues mutation and resets count on 200', async () => {
		await mod.enqueue(mutation());
		mockFetch(okResponse());
		await mod.sync(() => null);
		expect(get(mod.pendingCount)).toBe(0);
	});

	it('injects Bearer token from getToken into replayed request', async () => {
		await mod.enqueue(mutation());
		const fetchMock = mockFetch(okResponse());
		await mod.sync(() => 'my-token');
		const headers = fetchMock.mock.calls[0][1].headers as Record<string, string>;
		expect(headers['Authorization']).toBe('Bearer my-token');
	});

	it('omits Authorization header when getToken returns null', async () => {
		await mod.enqueue(mutation());
		const fetchMock = mockFetch(okResponse());
		await mod.sync(() => null);
		const headers = fetchMock.mock.calls[0][1].headers as Record<string, string>;
		expect(headers['Authorization']).toBeUndefined();
	});

	it('dequeues on 4xx permanent client error (except 401)', async () => {
		await mod.enqueue(mutation());
		mockFetch(clientError(422));
		await mod.sync(() => null);
		expect(get(mod.pendingCount)).toBe(0);
	});

	it('processes multiple mutations in FIFO order', async () => {
		await mod.enqueue(mutation('/api/v1/items/1', 'PUT'));
		await mod.enqueue(mutation('/api/v1/items/2', 'DELETE', null));
		const fetchMock = mockFetch(okResponse(), okResponse());
		await mod.sync(() => null);
		expect(get(mod.pendingCount)).toBe(0);
		expect(fetchMock.mock.calls[0][0]).toBe('/api/v1/items/1');
		expect(fetchMock.mock.calls[1][0]).toBe('/api/v1/items/2');
	});
});

// ── sync — transient failure ───────────────────────────────────────────────

describe('sync — transient failure', () => {
	it('keeps mutation in queue on 5xx', async () => {
		await mod.enqueue(mutation());
		mockFetch(serverError());
		await mod.sync(() => null);
		expect(get(mod.pendingCount)).toBe(1);
	});

	it('keeps mutation in queue on network error', async () => {
		await mod.enqueue(mutation());
		vi.stubGlobal('fetch', vi.fn().mockRejectedValueOnce(new Error('Network error')));
		await mod.sync(() => null);
		expect(get(mod.pendingCount)).toBe(1);
	});
});

// ── sync — 401 handling ────────────────────────────────────────────────────

describe('sync — 401', () => {
	it('stops processing remaining mutations on 401', async () => {
		await mod.enqueue(mutation('/api/v1/items/1', 'POST'));
		await mod.enqueue(mutation('/api/v1/items/2', 'POST'));
		const fetchMock = mockFetch({ ok: false, status: 401 } as Partial<Response>);
		await mod.sync(() => null);
		// Only one fetch call — stopped after first 401
		expect(fetchMock).toHaveBeenCalledTimes(1);
		// Both mutations still in queue
		expect(get(mod.pendingCount)).toBe(2);
	});
});

// ── sync — max attempts ────────────────────────────────────────────────────

describe('sync — max attempts', () => {
	it('drops mutation that has reached MAX_ATTEMPTS (20)', async () => {
		await mod.enqueue(mutation());
		// Fail 20 times to reach the limit
		for (let i = 0; i < 20; i++) {
			vi.stubGlobal('fetch', vi.fn().mockResolvedValueOnce(serverError()));
			await mod.sync(() => null);
		}
		// On the 21st call the mutation has attempts == 20 and is dropped without retrying
		const fetchMock = vi.fn();
		vi.stubGlobal('fetch', fetchMock);
		await mod.sync(() => null);
		expect(fetchMock).not.toHaveBeenCalled();
		expect(get(mod.pendingCount)).toBe(0);
	});
});

// ── sync — offline guard ───────────────────────────────────────────────────

describe('sync — offline', () => {
	it('returns immediately when offline without calling fetch', async () => {
		Object.defineProperty(navigator, 'onLine', { value: false, configurable: true });
		await mod.enqueue(mutation());
		const fetchMock = vi.fn();
		vi.stubGlobal('fetch', fetchMock);
		await mod.sync(() => null);
		expect(fetchMock).not.toHaveBeenCalled();
		expect(get(mod.pendingCount)).toBe(1);
	});
});

// ── sync — concurrency guard ───────────────────────────────────────────────

describe('sync — concurrency guard', () => {
	it('second concurrent sync call is a no-op', async () => {
		await mod.enqueue(mutation());
		let resolveFetch!: (r: Partial<Response>) => void;
		const slowFetch = vi.fn().mockReturnValue(
			new Promise<Partial<Response>>(r => { resolveFetch = r; })
		);
		vi.stubGlobal('fetch', slowFetch);

		const first = mod.sync(() => null);
		// Launch second sync while first is in flight
		const second = mod.sync(() => null);
		await second; // returns immediately

		resolveFetch(okResponse());
		await first;

		expect(slowFetch).toHaveBeenCalledTimes(1);
	});
});

// ── clear ──────────────────────────────────────────────────────────────────

describe('clear', () => {
	it('empties the queue and resets pendingCount to 0', async () => {
		await mod.enqueue(mutation());
		await mod.enqueue(mutation());
		expect(get(mod.pendingCount)).toBe(2);
		await mod.clear();
		expect(get(mod.pendingCount)).toBe(0);
	});

	it('subsequent sync processes no mutations after clear', async () => {
		await mod.enqueue(mutation());
		await mod.clear();
		const fetchMock = vi.fn();
		vi.stubGlobal('fetch', fetchMock);
		await mod.sync(() => null);
		expect(fetchMock).not.toHaveBeenCalled();
	});
});
