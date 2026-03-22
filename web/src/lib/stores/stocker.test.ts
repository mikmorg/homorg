import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import {
	stockerStore,
	setSession,
	clearSession,
	setContext,
	addRecentItem,
	setError,
	setPendingCount,
	markSynced,
	hasActiveSession,
	activeSession,
	activeContext,
	recentItems
} from './stocker';
import type { ScanSession, Item } from '$api/types';

function makeSession(overrides: Partial<ScanSession> = {}): ScanSession {
	return {
		id: 'session-1',
		user_id: 'user-1',
		active_container_id: null,
		started_at: '2024-01-01T00:00:00Z',
		ended_at: null,
		items_scanned: 0,
		items_created: 0,
		items_moved: 0,
		items_errored: 0,
		device_id: null,
		notes: null,
		...overrides
	};
}

function makeItem(overrides: Partial<Item> = {}): Item {
	return {
		id: 'item-1',
		system_barcode: 'HOM-000001',
		node_id: 'node-1',
		name: 'Test Item',
		description: null,
		category: null,
		category_id: null,
		tags: [],
		is_container: false,
		container_path: null,
		parent_id: null,
		coordinate: null,
		location_schema: null,
		max_capacity_cc: null,
		max_weight_grams: null,
		container_type_id: null,
		dimensions: null,
		weight_grams: null,
		is_fungible: false,
		fungible_quantity: null,
		fungible_unit: null,
		external_codes: [],
		condition: null,
		acquisition_date: null,
		acquisition_cost: null,
		current_value: null,
		depreciation_rate: null,
		warranty_expiry: null,
		currency: null,
		metadata: {},
		images: [],
		is_deleted: false,
		deleted_at: null,
		created_at: '2024-01-01T00:00:00Z',
		updated_at: '2024-01-01T00:00:00Z',
		created_by: null,
		updated_by: null,
		classification_confidence: null,
		needs_review: false,
		ai_description: null,
		...overrides
	};
}

describe('stockerStore', () => {
	beforeEach(() => {
		clearSession();
	});

	it('starts with no active session', () => {
		expect(get(hasActiveSession)).toBe(false);
		expect(get(activeSession)).toBeNull();
	});

	it('setSession activates a session', () => {
		const session = makeSession();
		setSession(session);
		expect(get(hasActiveSession)).toBe(true);
		expect(get(activeSession)?.id).toBe('session-1');
	});

	it('clearSession resets state', () => {
		setSession(makeSession());
		clearSession();
		expect(get(hasActiveSession)).toBe(false);
		expect(get(activeContext).containerId).toBeNull();
		expect(get(recentItems)).toEqual([]);
	});

	it('setContext updates context', () => {
		setContext({
			containerId: 'container-1',
			containerName: 'Box A',
			containerBarcode: 'HOM-000010'
		});
		const ctx = get(activeContext);
		expect(ctx.containerId).toBe('container-1');
		expect(ctx.containerName).toBe('Box A');
		expect(ctx.containerBarcode).toBe('HOM-000010');
	});

	it('addRecentItem adds items newest first', () => {
		addRecentItem(makeItem({ id: 'item-1', name: 'First' }));
		addRecentItem(makeItem({ id: 'item-2', name: 'Second' }));
		const items = get(recentItems);
		expect(items).toHaveLength(2);
		expect(items[0].name).toBe('Second');
		expect(items[1].name).toBe('First');
	});

	it('addRecentItem caps at 50', () => {
		for (let i = 0; i < 55; i++) {
			addRecentItem(makeItem({ id: `item-${i}` }));
		}
		expect(get(recentItems)).toHaveLength(50);
	});

	it('setPendingCount and markSynced', () => {
		setPendingCount(5);
		expect(get(stockerStore).pendingCount).toBe(5);
		markSynced();
		expect(get(stockerStore).pendingCount).toBe(0);
		expect(get(stockerStore).lastSyncAt).not.toBeNull();
	});

	it('setError sets and clears error', () => {
		setError('Something went wrong');
		expect(get(stockerStore).error).toBe('Something went wrong');
		setError(null);
		expect(get(stockerStore).error).toBeNull();
	});
});
