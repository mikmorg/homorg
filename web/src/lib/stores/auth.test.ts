import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import { authStore, isAuthenticated, isAdmin, isMember, currentUser, getAccessToken } from './auth';
import type { AuthResponse } from '$api/types';

// Mock localStorage
const storage = new Map<string, string>();
Object.defineProperty(globalThis, 'localStorage', {
	value: {
		getItem: (key: string) => storage.get(key) ?? null,
		setItem: (key: string, value: string) => storage.set(key, value),
		removeItem: (key: string) => storage.delete(key),
		clear: () => storage.clear()
	}
});

function makeAuthResponse(role: 'admin' | 'member' | 'readonly' = 'admin'): AuthResponse {
	return {
		access_token: 'test-access-token',
		refresh_token: 'test-refresh-token',
		expires_in: 3600,
		user: {
			id: 'user-1',
			username: 'testuser',
			display_name: 'Test User',
			role,
			is_active: true,
			container_id: 'container-1',
			created_at: '2024-01-01T00:00:00Z'
		}
	};
}

describe('authStore', () => {
	beforeEach(() => {
		storage.clear();
		authStore.clear();
	});

	it('starts as null when no stored auth', () => {
		expect(get(authStore)).toBeNull();
	});

	it('isAuthenticated is false when no auth', () => {
		expect(get(isAuthenticated)).toBe(false);
	});

	it('sets auth state from AuthResponse', () => {
		const response = makeAuthResponse();
		authStore.set(response);
		const state = get(authStore);
		expect(state).not.toBeNull();
		expect(state!.access_token).toBe('test-access-token');
		expect(state!.refresh_token).toBe('test-refresh-token');
		expect(state!.user.username).toBe('testuser');
	});

	it('isAuthenticated is true after set', () => {
		authStore.set(makeAuthResponse());
		expect(get(isAuthenticated)).toBe(true);
	});

	it('currentUser reflects the user', () => {
		authStore.set(makeAuthResponse());
		const user = get(currentUser);
		expect(user).not.toBeNull();
		expect(user!.username).toBe('testuser');
		expect(user!.role).toBe('admin');
	});

	it('isAdmin is true for admin role', () => {
		authStore.set(makeAuthResponse('admin'));
		expect(get(isAdmin)).toBe(true);
	});

	it('isAdmin is false for member role', () => {
		authStore.set(makeAuthResponse('member'));
		expect(get(isAdmin)).toBe(false);
	});

	it('isMember is true for admin', () => {
		authStore.set(makeAuthResponse('admin'));
		expect(get(isMember)).toBe(true);
	});

	it('isMember is true for member', () => {
		authStore.set(makeAuthResponse('member'));
		expect(get(isMember)).toBe(true);
	});

	it('isMember is false for readonly', () => {
		authStore.set(makeAuthResponse('readonly'));
		expect(get(isMember)).toBe(false);
	});

	it('clear resets state to null', () => {
		authStore.set(makeAuthResponse());
		expect(get(isAuthenticated)).toBe(true);
		authStore.clear();
		expect(get(isAuthenticated)).toBe(false);
		expect(get(authStore)).toBeNull();
	});

	it('getAccessToken returns token when authenticated', () => {
		authStore.set(makeAuthResponse());
		expect(getAccessToken()).toBe('test-access-token');
	});

	it('getAccessToken returns null when not authenticated', () => {
		expect(getAccessToken()).toBeNull();
	});

	it('persists to localStorage on set', () => {
		authStore.set(makeAuthResponse());
		const stored = storage.get('homorg_auth');
		expect(stored).toBeTruthy();
		const parsed = JSON.parse(stored!);
		expect(parsed.access_token).toBe('test-access-token');
	});

	it('clears localStorage on clear', () => {
		authStore.set(makeAuthResponse());
		expect(storage.has('homorg_auth')).toBe(true);
		authStore.clear();
		expect(storage.has('homorg_auth')).toBe(false);
	});

	it('updateUser updates the user but keeps tokens', () => {
		authStore.set(makeAuthResponse());
		authStore.updateUser({
			id: 'user-1',
			username: 'testuser',
			display_name: 'Updated Name',
			role: 'member',
			is_active: true,
			container_id: 'container-1',
			created_at: '2024-01-01T00:00:00Z'
		});
		const state = get(authStore);
		expect(state!.user.display_name).toBe('Updated Name');
		expect(state!.user.role).toBe('member');
		expect(state!.access_token).toBe('test-access-token');
	});
});
