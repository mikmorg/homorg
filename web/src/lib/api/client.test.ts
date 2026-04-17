import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';
import { authStore } from '$stores/auth.js';
import { ApiClientError, QueuedError, items, auth, containers, barcodes } from './client.js';
import type { AuthResponse } from './types.js';

vi.mock('$offline/queue.js', () => ({
	enqueue: vi.fn().mockResolvedValue(undefined),
	pendingCount: { subscribe: vi.fn(() => () => {}) },
	sync: vi.fn(),
	registerSyncListeners: vi.fn(() => () => {}),
	clear: vi.fn()
}));
import { enqueue } from '$offline/queue.js';

// ── Helpers ────────────────────────────────────────────────────────────────

const TEST_USER = {
	id: 'user-1', username: 'tester', role: 'member' as const,
	display_name: null, created_at: '2024-01-01T00:00:00Z',
	is_active: true, container_id: null
};

const AUTH_RESP: AuthResponse = {
	access_token: 'access-tok', refresh_token: 'refresh-tok',
	expires_in: 3600, user: TEST_USER
};

const NEW_AUTH_RESP: AuthResponse = {
	access_token: 'new-access-tok', refresh_token: 'new-refresh-tok',
	expires_in: 3600, user: TEST_USER
};

function mockFetch(...responses: Partial<Response>[]) {
	const fn = vi.fn();
	for (const r of responses) {
		fn.mockResolvedValueOnce(r);
	}
	vi.stubGlobal('fetch', fn);
	return fn;
}

function jsonResponse(body: unknown, status = 200): Partial<Response> {
	const text = JSON.stringify(body);
	return { ok: status >= 200 && status < 300, status, text: () => Promise.resolve(text), json: () => Promise.resolve(body) } as Partial<Response>;
}

function emptyResponse(status = 204): Partial<Response> {
	return { ok: true, status, text: () => Promise.resolve(''), json: () => Promise.reject(new Error('no body')) } as Partial<Response>;
}

function errorResponse(status: number, body: unknown, statusText = 'Error'): Partial<Response> {
	return { ok: false, status, statusText, text: () => Promise.resolve(JSON.stringify(body)), json: () => Promise.resolve(body) } as Partial<Response>;
}

function networkErrorFetch(message = 'Network error') {
	vi.stubGlobal('fetch', vi.fn().mockRejectedValueOnce(new Error(message)));
}

beforeEach(() => {
	authStore.clear();
	vi.restoreAllMocks();
});

// ── Basic request behaviour ────────────────────────────────────────────────

describe('successful responses', () => {
	it('returns parsed JSON on 200', async () => {
		mockFetch(jsonResponse({ id: 'abc', name: 'Wrench' }));
		const result = await items.get('abc');
		expect((result as { id: string }).id).toBe('abc');
	});

	it('returns undefined on 204 No Content', async () => {
		mockFetch(emptyResponse(204));
		const result = await items.restore('abc');
		expect(result).toBeUndefined();
	});

	it('returns undefined when body is empty string', async () => {
		mockFetch(emptyResponse(200));
		const result = await items.get('abc');
		expect(result).toBeUndefined();
	});
});

// ── Error handling ─────────────────────────────────────────────────────────

describe('error handling', () => {
	it('throws ApiClientError with status and message on error JSON', async () => {
		mockFetch(errorResponse(404, { message: 'Item not found' }));
		await expect(items.get('missing')).rejects.toMatchObject({
			error: { status: 404, message: 'Item not found' }
		});
	});

	it('throws ApiClientError using statusText when body is invalid JSON', async () => {
		vi.stubGlobal('fetch', vi.fn().mockResolvedValueOnce({
			ok: false, status: 500, statusText: 'Internal Server Error',
			json: () => Promise.reject(new SyntaxError('bad json')),
		} as Partial<Response>));
		await expect(items.get('x')).rejects.toMatchObject({
			error: { status: 500, message: 'Internal Server Error' }
		});
	});

	it('ApiClientError is instanceof Error', async () => {
		mockFetch(errorResponse(403, { message: 'Forbidden' }));
		try {
			await items.get('x');
			expect.fail('should throw');
		} catch (e) {
			expect(e).toBeInstanceOf(ApiClientError);
			expect(e).toBeInstanceOf(Error);
			expect((e as ApiClientError).message).toBe('Forbidden');
		}
	});
});

// ── Authorization header ───────────────────────────────────────────────────

describe('authorization header', () => {
	it('sends Authorization header when access_token is present', async () => {
		authStore.set(AUTH_RESP);
		const fetchMock = mockFetch(jsonResponse({}));
		await items.get('abc');
		const calledWith = fetchMock.mock.calls[0];
		const headers: Headers = calledWith[1].headers;
		expect(headers.get('Authorization')).toBe('Bearer access-tok');
	});

	it('omits Authorization header when not authenticated', async () => {
		const fetchMock = mockFetch(jsonResponse({}));
		await auth.refresh('some-refresh-token');
		const calledWith = fetchMock.mock.calls[0];
		const headers: Headers = calledWith[1].headers;
		expect(headers.get('Authorization')).toBeNull();
	});
});

// ── Query parameter serialization ─────────────────────────────────────────

describe('query parameters', () => {
	it('appends defined params to URL', async () => {
		const fetchMock = mockFetch(jsonResponse([]));
		await containers.children('root', { limit: 50, sort_by: 'name', sort_dir: 'asc' });
		const url: string = fetchMock.mock.calls[0][0];
		expect(url).toContain('limit=50');
		expect(url).toContain('sort_by=name');
		expect(url).toContain('sort_dir=asc');
	});

	it('omits null and undefined param values', async () => {
		const fetchMock = mockFetch(jsonResponse([]));
		await containers.children('root', { limit: 50, cursor: undefined, sort_by: 'name', sort_dir: 'asc' });
		const url: string = fetchMock.mock.calls[0][0];
		expect(url).not.toContain('cursor');
	});

	it('builds URL without query string when no params defined', async () => {
		const fetchMock = mockFetch(jsonResponse([]));
		await containers.ancestors('container-1');
		const url: string = fetchMock.mock.calls[0][0];
		expect(url).toBe('/api/v1/containers/container-1/ancestors');
	});
});

// ── Content-Type header ───────────────────────────────────────────────────

describe('Content-Type header', () => {
	it('sets application/json by default for JSON body', async () => {
		const fetchMock = mockFetch(jsonResponse({ access_token: 't', refresh_token: 'r', expires_in: 3600, user: TEST_USER }));
		await auth.login({ username: 'u', password: 'p' });
		const headers: Headers = fetchMock.mock.calls[0][1].headers;
		expect(headers.get('Content-Type')).toBe('application/json');
	});

	it('does not override Content-Type for FormData (browser sets boundary)', async () => {
		authStore.set(AUTH_RESP);
		const fetchMock = mockFetch(jsonResponse({}));
		const file = new File(['data'], 'test.png', { type: 'image/png' });
		await items.uploadImage('item-1', file);
		const headers: Headers = fetchMock.mock.calls[0][1].headers;
		// fetch sets multipart boundary automatically when Content-Type absent
		expect(headers.get('Content-Type')).toBeNull();
	});
});

// ── Token refresh (401 handling) ───────────────────────────────────────────

describe('token refresh on 401', () => {
	it('retries with new token when 401 and refresh succeeds', async () => {
		authStore.set(AUTH_RESP);
		const fetchMock = vi.fn()
			// First call: 401
			.mockResolvedValueOnce({ ok: false, status: 401, statusText: 'Unauthorized' } as Partial<Response>)
			// Refresh call: returns new tokens
			.mockResolvedValueOnce(jsonResponse(NEW_AUTH_RESP))
			// Retry: success
			.mockResolvedValueOnce(jsonResponse({ id: 'item-1' }));
		vi.stubGlobal('fetch', fetchMock);

		const result = await items.get('item-1');
		expect((result as { id: string }).id).toBe('item-1');
		expect(fetchMock).toHaveBeenCalledTimes(3);
		// Third call should use new token
		const retryHeaders: Headers = fetchMock.mock.calls[2][1].headers;
		expect(retryHeaders.get('Authorization')).toBe('Bearer new-access-tok');
	});

	it('clears authStore and throws when refresh returns non-ok', async () => {
		authStore.set(AUTH_RESP);
		vi.stubGlobal('fetch', vi.fn()
			.mockResolvedValueOnce({ ok: false, status: 401, statusText: 'Unauthorized' } as Partial<Response>)
			.mockResolvedValueOnce({ ok: false, status: 401, statusText: 'Unauthorized' } as Partial<Response>)
		);

		await expect(items.get('x')).rejects.toThrow('Session expired');
		expect(get(authStore)).toBeNull();
	});

	it('does not retry when no refresh_token is present', async () => {
		// No auth state — no retry
		const fetchMock = mockFetch(errorResponse(401, { message: 'Unauthorized' }));
		await expect(items.get('x')).rejects.toMatchObject({ error: { status: 401 } });
		expect(fetchMock).toHaveBeenCalledTimes(1);
	});

	it('throws retry-upload error and still refreshes for FormData 401', async () => {
		authStore.set(AUTH_RESP);
		vi.stubGlobal('fetch', vi.fn()
			.mockResolvedValueOnce({ ok: false, status: 401, statusText: 'Unauthorized' } as Partial<Response>)
			.mockResolvedValueOnce(jsonResponse(NEW_AUTH_RESP))
		);

		const file = new File(['data'], 'img.png', { type: 'image/png' });
		await expect(items.uploadImage('item-1', file)).rejects.toMatchObject({
			error: { status: 401, message: 'Session refreshed — please retry the upload' }
		});
		// Auth store should be updated with new token
		expect(get(authStore)?.access_token).toBe('new-access-tok');
	});
});

// ── Concurrent 401 refresh (CL-1 refreshQueue) ────────────────────────────────

describe('concurrent token refresh (CL-1)', () => {
	it('fires only one refresh call when two requests 401 simultaneously', async () => {
		authStore.set(AUTH_RESP);

		// Set up a slow refresh so we can have two 401s in flight at once.
		let resolveRefresh!: (r: Partial<Response>) => void;
		const refreshPromise = new Promise<Partial<Response>>((r) => { resolveRefresh = r; });

		const fetchMock = vi.fn()
			// Both concurrent requests get 401
			.mockResolvedValueOnce({ ok: false, status: 401, statusText: 'Unauthorized' } as Partial<Response>)
			.mockResolvedValueOnce({ ok: false, status: 401, statusText: 'Unauthorized' } as Partial<Response>)
			// One refresh call (slow)
			.mockReturnValueOnce(refreshPromise)
			// Both retries succeed
			.mockResolvedValueOnce(jsonResponse({ id: 'item-1' }))
			.mockResolvedValueOnce(jsonResponse({ id: 'item-2' }));
		vi.stubGlobal('fetch', fetchMock);

		// Launch two requests in parallel before refresh completes
		const p1 = items.get('item-1');
		const p2 = items.get('item-2');

		// Allow the initial 401 responses to land
		await Promise.resolve();
		await Promise.resolve();

		// Now resolve the refresh
		resolveRefresh(jsonResponse(NEW_AUTH_RESP));

		const [r1, r2] = await Promise.all([p1, p2]);
		expect((r1 as { id: string }).id).toBe('item-1');
		expect((r2 as { id: string }).id).toBe('item-2');

		// Exactly one refresh call should have fired
		const refreshCalls = fetchMock.mock.calls.filter((args: unknown[]) =>
			typeof args[0] === 'string' && args[0].includes('/auth/refresh')
		);
		expect(refreshCalls).toHaveLength(1);
	});

	it('all queued callers reject when refresh fails', async () => {
		authStore.set(AUTH_RESP);

		vi.stubGlobal('fetch', vi.fn()
			.mockResolvedValueOnce({ ok: false, status: 401, statusText: 'Unauthorized' } as Partial<Response>)
			.mockResolvedValueOnce({ ok: false, status: 401, statusText: 'Unauthorized' } as Partial<Response>)
			// Refresh itself fails
			.mockResolvedValueOnce({ ok: false, status: 401, statusText: 'Unauthorized' } as Partial<Response>));

		const p1 = items.get('item-1');
		const p2 = items.get('item-2');

		await expect(Promise.all([p1, p2])).rejects.toThrow('Session expired');
		expect(get(authStore)).toBeNull();
	});
});

// ── requestBlob (PDF downloads) ───────────────────────────────────────────────

describe('requestBlob', () => {
	it('returns a Blob on successful response', async () => {
		authStore.set(AUTH_RESP);
		const pdfBytes = new Uint8Array([0x25, 0x50, 0x44, 0x46]); // %PDF
		vi.stubGlobal('fetch', vi.fn().mockResolvedValueOnce({
			ok: true,
			status: 200,
			blob: () => Promise.resolve(new Blob([pdfBytes], { type: 'application/pdf' }))
		} as Partial<Response>));

		const blob = await barcodes.downloadLabels(10, '30-up');
		expect(blob).toBeInstanceOf(Blob);
	});

	it('retries with new token when 401 and refresh succeeds', async () => {
		authStore.set(AUTH_RESP);
		const pdfBlob = new Blob(['pdf-data'], { type: 'application/pdf' });

		vi.stubGlobal('fetch', vi.fn()
			.mockResolvedValueOnce({ ok: false, status: 401, statusText: 'Unauthorized' } as Partial<Response>)
			.mockResolvedValueOnce(jsonResponse(NEW_AUTH_RESP))
			.mockResolvedValueOnce({
				ok: true, status: 200,
				blob: () => Promise.resolve(pdfBlob)
			} as Partial<Response>)
		);

		const result = await barcodes.downloadLabels(5, '30-up');
		expect(result).toBeInstanceOf(Blob);
		expect(get(authStore)?.access_token).toBe('new-access-tok');
	});

	it('throws ApiClientError when response is not ok', async () => {
		authStore.set(AUTH_RESP);
		vi.stubGlobal('fetch', vi.fn().mockResolvedValueOnce({
			ok: false,
			status: 403,
			statusText: 'Forbidden',
			json: () => Promise.resolve({ message: 'Not an admin' })
		} as Partial<Response>));

		await expect(barcodes.downloadLabels(10, '30-up')).rejects.toMatchObject({
			error: { status: 403, message: 'Not an admin' }
		});
	});
});

// ── Offline queue integration ─────────────────────────────────────────────────

describe('offline queue integration', () => {
	beforeEach(() => {
		vi.mocked(enqueue).mockClear();
	});

	it('enqueues and throws QueuedError when POST fails with network error', async () => {
		authStore.set(AUTH_RESP);
		vi.stubGlobal('fetch', vi.fn().mockRejectedValueOnce(new Error('Network error')));

		await expect(items.create({ name: 'Wrench' } as never)).rejects.toBeInstanceOf(QueuedError);
		expect(enqueue).toHaveBeenCalledOnce();
		expect(vi.mocked(enqueue).mock.calls[0][0]).toMatchObject({
			url: '/api/v1/items',
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ name: 'Wrench' })
		});
	});

	it('includes query params in enqueued URL', async () => {
		authStore.set(AUTH_RESP);
		vi.stubGlobal('fetch', vi.fn().mockRejectedValueOnce(new Error('Network error')));

		await expect(
			items.adjustQuantity('item-1', { delta: 1, unit: 'units' } as never)
		).rejects.toBeInstanceOf(QueuedError);
		expect(vi.mocked(enqueue).mock.calls[0][0]).toMatchObject({
			url: '/api/v1/items/item-1/quantity',
			method: 'POST'
		});
	});

	it('does not enqueue GET requests on network error', async () => {
		vi.stubGlobal('fetch', vi.fn().mockRejectedValueOnce(new Error('Network error')));

		await expect(items.get('item-1')).rejects.toThrow('Network error');
		expect(enqueue).not.toHaveBeenCalled();
	});

	it('does not enqueue FormData (file upload) on network error', async () => {
		authStore.set(AUTH_RESP);
		vi.stubGlobal('fetch', vi.fn().mockRejectedValueOnce(new Error('Network error')));

		const file = new File(['data'], 'img.png', { type: 'image/png' });
		await expect(items.uploadImage('item-1', file)).rejects.toThrow('Network error');
		expect(enqueue).not.toHaveBeenCalled();
	});

	it('does not enqueue auth endpoints on network error', async () => {
		vi.stubGlobal('fetch', vi.fn().mockRejectedValueOnce(new Error('Network error')));

		await expect(auth.login({ username: 'u', password: 'p' })).rejects.toThrow('Network error');
		expect(enqueue).not.toHaveBeenCalled();
	});

	it('does not enqueue DELETE on auth path on network error', async () => {
		vi.stubGlobal('fetch', vi.fn().mockRejectedValueOnce(new Error('Network error')));

		await expect(auth.logout('refresh-tok')).rejects.toThrow('Network error');
		expect(enqueue).not.toHaveBeenCalled();
	});
});

describe('request deduplication (regression check)', () => {
	it('allows tracking request count for monitoring excessive API calls', async () => {
		const fetchFn = vi.fn();
		const testItem = { id: 'item-1', name: 'Test' };

		// Simulate: load item, then check if it's already loaded before fetching again
		fetchFn.mockResolvedValueOnce(jsonResponse(testItem));
		fetchFn.mockResolvedValueOnce(jsonResponse(testItem));

		vi.stubGlobal('fetch', fetchFn);

		// First call should fetch
		const result1 = await items.get('item-1');
		expect(result1).toEqual(testItem);
		expect(fetchFn).toHaveBeenCalledTimes(1);

		// Ideally this should reuse cached result, but currently fetches again
		// This test documents the current behavior; can be improved to cache
		const result2 = await items.get('item-1');
		expect(result2).toEqual(testItem);
		expect(fetchFn).toHaveBeenCalledTimes(2);

		// TODO: When request caching is implemented, this should be 1, not 2
		// This would catch regressions where we add unnecessary duplicate fetches
	});

	it('detects when multiple endpoints fetch same data without dedup', async () => {
		const fetchFn = vi.fn();
		vi.stubGlobal('fetch', fetchFn);

		const testItem = { id: 'item-1', name: 'Container' };
		fetchFn.mockResolvedValueOnce(jsonResponse(testItem));
		fetchFn.mockResolvedValueOnce(jsonResponse(testItem));
		fetchFn.mockResolvedValueOnce(jsonResponse(testItem));

		// Simulating stocker page behavior: fetch item multiple times
		const item1 = await items.get('item-1');
		const item2 = await items.get('item-1'); // Same ID
		const item3 = await items.get('item-1'); // Same ID again

		// This demonstrates 3 redundant fetches; should be 1 with dedup
		expect(fetchFn).toHaveBeenCalledTimes(3);
		expect([item1, item2, item3]).toEqual([testItem, testItem, testItem]);

		// NOTE: This test is a regression baseline. If this count increases further,
		// it means code is making even more redundant requests than now.
	});
});
