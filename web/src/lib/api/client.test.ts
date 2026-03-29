import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';
import { authStore } from '$stores/auth.js';
import { ApiClientError, items, auth, containers } from './client.js';
import type { AuthResponse } from './types.js';

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
