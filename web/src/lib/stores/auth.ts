import { writable, derived, get } from 'svelte/store';
import type { AuthResponse, UserPublic } from '$api/types.js';

interface AuthState {
	access_token: string;
	refresh_token: string;
	expires_at: number; // unix ms
	user: UserPublic;
}

const STORAGE_KEY = 'homorg_auth';

function loadFromStorage(): AuthState | null {
	if (typeof localStorage === 'undefined') return null;
	try {
		const raw = localStorage.getItem(STORAGE_KEY);
		if (!raw) return null;
		return JSON.parse(raw) as AuthState;
	} catch {
		return null;
	}
}

function saveToStorage(state: AuthState | null) {
	if (typeof localStorage === 'undefined') return;
	if (state) {
		localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
	} else {
		localStorage.removeItem(STORAGE_KEY);
	}
}

function createAuthStore() {
	const { subscribe, set: _set, update } = writable<AuthState | null>(loadFromStorage());

	function set(response: AuthResponse) {
		const state: AuthState = {
			access_token: response.access_token,
			refresh_token: response.refresh_token,
			expires_at: Date.now() + response.expires_in * 1000,
			user: response.user
		};
		saveToStorage(state);
		_set(state);
	}

	function clear() {
		saveToStorage(null);
		_set(null);
	}

	function updateUser(user: UserPublic) {
		update((s) => {
			if (!s) return s;
			const next = { ...s, user };
			saveToStorage(next);
			return next;
		});
	}

	return { subscribe, set, clear, updateUser };
}

export const authStore = createAuthStore();

export const isAuthenticated = derived(authStore, ($auth) => $auth !== null);
export const currentUser = derived(authStore, ($auth) => $auth?.user ?? null);
export const isAdmin = derived(authStore, ($auth) => $auth?.user?.role === 'admin');
export const isMember = derived(
	authStore,
	($auth) => $auth?.user?.role === 'admin' || $auth?.user?.role === 'member'
);

export function getAccessToken(): string | null {
	return get(authStore)?.access_token ?? null;
}
