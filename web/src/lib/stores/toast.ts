import { writable } from 'svelte/store';

export interface ToastMessage {
	id: number;
	text: string;
	type: 'success' | 'error' | 'info';
	undoEventId?: string;
	_timerId?: ReturnType<typeof setTimeout>;
}

let idCounter = 0;
export const toasts = writable<ToastMessage[]>([]);

export function toast(text: string, type: ToastMessage['type'] = 'success', duration = 3000) {
	const id = ++idCounter;
	const timerId = setTimeout(() => {
		toasts.update((t) => t.filter((m) => m.id !== id));
	}, duration);
	toasts.update((t) => [...t, { id, text, type, _timerId: timerId }]);
}

export function toastWithUndo(text: string, eventId: string, duration = 5000) {
	const id = ++idCounter;
	const timerId = setTimeout(() => {
		toasts.update((t) => t.filter((m) => m.id !== id));
	}, duration);
	toasts.update((t) => [...t, { id, text, type: 'success', undoEventId: eventId, _timerId: timerId }]);
}

export function dismissToast(id: number) {
	toasts.update((t) => {
		const entry = t.find((m) => m.id === id);
		if (entry?._timerId) clearTimeout(entry._timerId);
		return t.filter((m) => m.id !== id);
	});
}
