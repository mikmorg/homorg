import {
	test as base,
	request,
	expect,
	type APIRequestContext
} from '@playwright/test';
import { readFileSync } from 'node:fs';
import { STORAGE_STATE } from './constants';

const BACKEND_BASE = 'http://localhost:8080/api/v1/';
export const ROOT_ID = '00000000-0000-0000-0000-000000000001';

function readAccessToken(): string {
	const raw = readFileSync(STORAGE_STATE, 'utf8');
	const state = JSON.parse(raw);
	for (const origin of state.origins ?? []) {
		for (const entry of origin.localStorage ?? []) {
			if (entry.name === 'homorg_auth') {
				const auth = JSON.parse(entry.value);
				if (auth?.access_token) return auth.access_token;
			}
		}
	}
	throw new Error(`No homorg_auth access_token found in ${STORAGE_STATE}`);
}

interface EventResponse {
	aggregate_id: string;
	[key: string]: unknown;
}

export class BackendApi {
	constructor(private readonly ctx: APIRequestContext) {}

	async createItem(body: Record<string, unknown>): Promise<string> {
		const res = await this.ctx.post('items', { data: body });
		if (!res.ok()) {
			throw new Error(`POST items ${res.status()}: ${await res.text()}`);
		}
		const ev = (await res.json()) as EventResponse;
		if (!ev.aggregate_id) throw new Error(`create_item: no aggregate_id in response`);
		return ev.aggregate_id;
	}

	createContainer(parentId: string, name: string, extra: Record<string, unknown> = {}) {
		return this.createItem({ parent_id: parentId, name, is_container: true, ...extra });
	}

	async deleteItem(id: string): Promise<void> {
		await this.ctx.delete(`items/${id}`);
	}

	async startSession(initialContainerId?: string): Promise<string> {
		const body: Record<string, unknown> = {};
		if (initialContainerId) body.initial_container_id = initialContainerId;
		const res = await this.ctx.post('stocker/sessions', { data: body });
		if (!res.ok()) {
			throw new Error(`POST stocker/sessions ${res.status()}: ${await res.text()}`);
		}
		const session = (await res.json()) as { id: string };
		return session.id;
	}

	async endSession(sessionId: string): Promise<void> {
		const res = await this.ctx.put(`stocker/sessions/${sessionId}/end`);
		if (!res.ok()) {
			throw new Error(`PUT stocker/sessions/${sessionId}/end ${res.status()}: ${await res.text()}`);
		}
	}

	async submitBatch(
		sessionId: string,
		events: Record<string, unknown>[]
	): Promise<Record<string, unknown>> {
		const res = await this.ctx.post(`stocker/sessions/${sessionId}/batch`, {
			data: { events }
		});
		if (!res.ok()) {
			throw new Error(
				`POST stocker/sessions/${sessionId}/batch ${res.status()}: ${await res.text()}`
			);
		}
		return (await res.json()) as Record<string, unknown>;
	}
}

type Fixtures = {
	api: BackendApi;
};

export const test = base.extend<Fixtures>({
	api: async ({}, use) => {
		const ctx = await request.newContext({
			baseURL: BACKEND_BASE,
			extraHTTPHeaders: { Authorization: `Bearer ${readAccessToken()}` }
		});
		await use(new BackendApi(ctx));
		await ctx.dispose();
	}
});

export { expect };
