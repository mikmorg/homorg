import type {
	AuthResponse, LoginRequest, RegisterRequest, SetupRequest, InviteResponse,
	Item, ItemSummary, AncestorEntry, ContainerStats,
	CreateItemRequest, UpdateItemRequest, MoveItemRequest,
	AdjustQuantityRequest, AssignBarcodeRequest,
	BarcodeResolution, GeneratedBarcode,
	ScanSession, StartSessionRequest, StockerBatchRequest, StockerBatchResponse,
	ChildrenParams, DescendantsParams, SearchParams,
	Category, Tag, ContainerType,
	StoredEvent, HealthResponse, StatsResponse,
	UserPublic, UpdateUserRequest, UpdateRoleRequest,
	ApiError
} from './types.js';
import { authStore } from '$stores/auth.js';
import { get } from 'svelte/store';

export class ApiClientError extends Error {
	constructor(public readonly error: ApiError) {
		super(error.message);
	}
}

const BASE = '/api/v1';
let isRefreshing = false;
let refreshQueue: Array<{ resolve: (token: string) => void; reject: (err: Error) => void }> = [];

async function refreshAndRetry(token: string): Promise<string> {
	if (isRefreshing) {
		return new Promise((resolve, reject) => refreshQueue.push({ resolve, reject }));
	}
	isRefreshing = true;
	try {
		const auth = get(authStore);
		if (!auth?.refresh_token) throw new Error('No refresh token');
		const resp = await fetch(`${BASE}/auth/refresh`, {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ refresh_token: auth.refresh_token })
		});
		if (!resp.ok) {
			authStore.clear();
			throw new Error('Session expired');
		}
		const data: AuthResponse = await resp.json();
		authStore.set(data);
		refreshQueue.forEach(({ resolve }) => resolve(data.access_token));
		return data.access_token;
	} catch (e) {
		// CL-1: Reject all queued callers so their Promises don't hang indefinitely
		// when a token refresh fails (e.g. expired refresh token, network error).
		const err = e instanceof Error ? e : new Error('Refresh failed');
		refreshQueue.forEach(({ reject }) => reject(err));
		throw err;
	} finally {
		isRefreshing = false;
		refreshQueue = [];
	}
}

async function request<T>(
	method: string,
	path: string,
	options: RequestInit & { params?: Record<string, unknown> } = {},
	retry = true
): Promise<T> {
	const { params, ...fetchOptions } = options;
	let url = `${BASE}${path}`;
	if (params) {
		const q = new URLSearchParams();
		for (const [k, v] of Object.entries(params)) {
			if (v !== undefined && v !== null) q.set(k, String(v));
		}
		const qs = q.toString();
		if (qs) url += `?${qs}`;
	}

	const auth = get(authStore);
	const headers = new Headers(fetchOptions.headers);
	if (!headers.has('Content-Type') && !(fetchOptions.body instanceof FormData)) {
		headers.set('Content-Type', 'application/json');
	}
	if (auth?.access_token) {
		headers.set('Authorization', `Bearer ${auth.access_token}`);
	}

	const resp = await fetch(url, { method, ...fetchOptions, headers });

	if (resp.status === 401 && retry && auth?.refresh_token) {
		const newToken = await refreshAndRetry(auth.access_token ?? '');
		headers.set('Authorization', `Bearer ${newToken}`);
		const retry$ = await fetch(url, { method, ...fetchOptions, headers });
		if (!retry$.ok) {
			const err = await retry$.json().catch(() => ({ message: retry$.statusText }));
			throw new ApiClientError({ status: retry$.status, message: err.message ?? retry$.statusText });
		}
		if (retry$.status === 204) return undefined as T;
		return retry$.json();
	}

	if (!resp.ok) {
		const err = await resp.json().catch(() => ({ message: resp.statusText }));
		throw new ApiClientError({ status: resp.status, message: err.message ?? resp.statusText });
	}

	if (resp.status === 204) return undefined as T;
	// Return empty array/undefined for 200 with no body
	const text = await resp.text();
	if (!text) return undefined as T;
	return JSON.parse(text);
}

const get$ = <T>(path: string, params?: Record<string, unknown>) =>
	request<T>('GET', path, { params });
const post$ = <T>(path: string, body?: unknown) =>
	request<T>('POST', path, { body: body !== undefined ? JSON.stringify(body) : undefined });
const put$ = <T>(path: string, body?: unknown) =>
	request<T>('PUT', path, { body: body !== undefined ? JSON.stringify(body) : undefined });
const del$ = <T>(path: string) =>
	request<T>('DELETE', path);

// ─── Auth ────────────────────────────────────────────────────────────────────

export const auth = {
	setup: (body: SetupRequest) => post$<AuthResponse>('/auth/setup', body),
	login: (body: LoginRequest) => post$<AuthResponse>('/auth/login', body),
	refresh: (refresh_token: string) => post$<AuthResponse>('/auth/refresh', { refresh_token }),
	logout: (refresh_token: string) => post$<void>('/auth/logout', { refresh_token }),
	me: () => get$<UserPublic>('/auth/me'),
	invite: () => post$<InviteResponse>('/auth/invite'),
	register: (body: RegisterRequest) => post$<AuthResponse>('/auth/register', body)
};

// ─── Items ───────────────────────────────────────────────────────────────────

export const items = {
	get: (id: string) => get$<Item>(`/items/${id}`),
	create: (body: CreateItemRequest) => post$<StoredEvent>('/items', body),
	update: (id: string, body: UpdateItemRequest) => put$<StoredEvent>(`/items/${id}`, body),
	delete: (id: string, reason?: string) =>
		request<void>('DELETE', `/items/${id}`, {
			body: reason ? JSON.stringify({ reason }) : undefined
		}),
	restore: (id: string) => post$<StoredEvent>(`/items/${id}/restore`),
	move: (id: string, body: MoveItemRequest) => post$<StoredEvent>(`/items/${id}/move`, body),
	history: (id: string, params?: { limit?: number; after_seq?: number }) =>
		get$<StoredEvent[]>(`/items/${id}/history`, params),
	uploadImage: (id: string, file: File, caption?: string, order?: number) => {
		const form = new FormData();
		form.append('file', file);
		if (caption) form.append('caption', caption);
		if (order !== undefined) form.append('order', String(order));
		return request<StoredEvent>('POST', `/items/${id}/images`, { body: form });
	},
	removeImage: (id: string, idx: number) => del$<StoredEvent>(`/items/${id}/images/${idx}`),
	addExternalCode: (id: string, code_type: string, value: string) =>
		post$<StoredEvent>(`/items/${id}/external-codes`, { type: code_type, value }),
	removeExternalCode: (id: string, code_type: string, value: string) =>
		del$<StoredEvent>(`/items/${id}/external-codes/${code_type}/${encodeURIComponent(value)}`),
	adjustQuantity: (id: string, body: AdjustQuantityRequest) =>
		post$<StoredEvent>(`/items/${id}/quantity`, body),
	assignBarcode: (id: string, body: AssignBarcodeRequest) =>
		post$<StoredEvent>(`/items/${id}/barcode`, body)
};

// ─── Containers ──────────────────────────────────────────────────────────────

export const containers = {
	children: (id: string, params?: ChildrenParams) =>
		get$<ItemSummary[]>(`/containers/${id}/children`, params as Record<string, unknown>),
	descendants: (id: string, params?: DescendantsParams) =>
		get$<ItemSummary[]>(`/containers/${id}/descendants`, params as Record<string, unknown>),
	ancestors: (id: string) => get$<AncestorEntry[]>(`/containers/${id}/ancestors`),
	stats: (id: string) => get$<ContainerStats>(`/containers/${id}/stats`),
	updateSchema: (id: string, schema: unknown, label_renames?: Record<string, string>) =>
		put$<StoredEvent>(`/containers/${id}/schema`, { schema, ...(label_renames && Object.keys(label_renames).length > 0 ? { label_renames } : {}) })
};

// ─── Barcodes ────────────────────────────────────────────────────────────────

export const barcodes = {
	generate: () => post$<GeneratedBarcode>('/barcodes/generate'),
	generateBatch: (count: number) =>
		post$<GeneratedBarcode[]>('/barcodes/generate-batch', { count }),
	resolve: (code: string) =>
		get$<BarcodeResolution>(`/barcodes/resolve/${encodeURIComponent(code)}`)
};

// ─── Stocker ─────────────────────────────────────────────────────────────────

export const stocker = {
	startSession: (body?: StartSessionRequest) => post$<ScanSession>('/stocker/sessions', body),
	listSessions: (limit = 20) =>
		get$<ScanSession[]>('/stocker/sessions', { limit }),
	getSession: (id: string) => get$<ScanSession>(`/stocker/sessions/${id}`),
	submitBatch: (id: string, body: StockerBatchRequest, atomic = false) =>
		request<StockerBatchResponse>('POST', `/stocker/sessions/${id}/batch`, {
			body: JSON.stringify(body),
			params: { atomic }
		}),
	endSession: (id: string) => put$<ScanSession>(`/stocker/sessions/${id}/end`)
};

// ─── Search ──────────────────────────────────────────────────────────────────

export const search = {
	query: (params: SearchParams) =>
		get$<ItemSummary[]>('/search', params as Record<string, unknown>)
};

// ─── Undo ────────────────────────────────────────────────────────────────────

export const undo = {
	single: (event_id: string) => post$<StoredEvent>(`/undo/event/${event_id}`),
	batch: (event_ids: string[]) => post$<StoredEvent[]>('/undo/batch', { event_ids })
};

// ─── Taxonomy ────────────────────────────────────────────────────────────────

export const categories = {
	list: () => get$<Category[]>('/categories'),
	get: (id: string) => get$<Category>(`/categories/${id}`),
	create: (name: string, description?: string, parent_category_id?: string) =>
		post$<Category>('/categories', { name, description, parent_category_id }),
	update: (id: string, body: Partial<Category>) => put$<Category>(`/categories/${id}`, body),
	delete: (id: string) => del$<void>(`/categories/${id}`)
};

export const tags = {
	list: () => get$<Tag[]>('/tags'),
	get: (id: string) => get$<Tag>(`/tags/${id}`),
	create: (name: string) => post$<Tag>('/tags', { name }),
	rename: (id: string, name: string) => put$<Tag>(`/tags/${id}/rename`, { name }),
	delete: (id: string) => del$<void>(`/tags/${id}`)
};

// ─── Container Types ─────────────────────────────────────────────────────────

export const containerTypes = {
	list: () => get$<ContainerType[]>('/container-types'),
	get: (id: string) => get$<ContainerType>(`/container-types/${id}`),
	create: (body: Partial<ContainerType>) => post$<ContainerType>('/container-types', body),
	update: (id: string, body: Partial<ContainerType>) =>
		put$<ContainerType>(`/container-types/${id}`, body),
	delete: (id: string) => del$<void>(`/container-types/${id}`)
};

// ─── Users (admin) ───────────────────────────────────────────────────────────

export const users = {
	list: () => get$<UserPublic[]>('/users'),
	get: (id: string) => get$<UserPublic>(`/users/${id}`),
	update: (id: string, body: UpdateUserRequest) => put$<UserPublic>(`/users/${id}`, body),
	deactivate: (id: string) => del$<void>(`/users/${id}`),
	updateRole: (id: string, role: UpdateRoleRequest) => put$<UserPublic>(`/users/${id}/role`, role)
};

// ─── System ──────────────────────────────────────────────────────────────────

export const system = {
	health: () => get$<HealthResponse>('/health'),
	stats: () => get$<StatsResponse>('/stats'),
	rebuildProjections: () => post$<void>('/admin/rebuild-projections'),
	rebuildStatus: () => get$<{ in_progress: boolean }>('/admin/rebuild-status')
};

// ─── Convenience aggregate ───────────────────────────────────────────────────
export const api = { auth, items, containers, barcodes, stocker, search, undo, categories, tags, containerTypes, users, system };
