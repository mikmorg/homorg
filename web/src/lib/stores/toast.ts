import { writable } from 'svelte/store';

export interface ToastMessage {
	id: number;
	text: string;
	type: 'success' | 'error' | 'info';
	undoEventId?: string;
}

let idCounter = 0;
export const toasts = writable<ToastMessage[]>([]);

export function toast(text: string, type: ToastMessage['type'] = 'success', duration = 3000) {
	const id = ++idCounter;
	toasts.update((t) => [...t, { id, text, type }]);
	setTimeout(() => {
		toasts.update((t) => t.filter((m) => m.id !== id));
	}, duration);
}

export function toastWithUndo(text: string, eventId: string, duration = 5000) {
	const id = ++idCounter;
	toasts.update((t) => [...t, { id, text, type: 'success', undoEventId: eventId }]);
	setTimeout(() => {
		toasts.update((t) => t.filter((m) => m.id !== id));
	}, duration);
}

export function dismissToast(id: number) {
	toasts.update((t) => t.filter((m) => m.id !== id));
}
