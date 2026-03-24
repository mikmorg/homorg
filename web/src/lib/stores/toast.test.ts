import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { get } from 'svelte/store';
import { toasts, toast, toastWithUndo, dismissToast } from './toast';

beforeEach(() => {
	// Clear all toasts between tests
	toasts.set([]);
	vi.useFakeTimers();
});

afterEach(() => {
	vi.useRealTimers();
});

describe('toast', () => {
	it('adds a message to the toasts store', () => {
		toast('Hello', 'success');
		expect(get(toasts)).toHaveLength(1);
		expect(get(toasts)[0]).toMatchObject({ text: 'Hello', type: 'success' });
	});

	it('assigns a unique id', () => {
		toast('First', 'info');
		toast('Second', 'info');
		const msgs = get(toasts);
		expect(msgs[0].id).not.toBe(msgs[1].id);
	});

	it('defaults to success type', () => {
		toast('Default');
		expect(get(toasts)[0].type).toBe('success');
	});

	it('accepts error type', () => {
		toast('Oops', 'error');
		expect(get(toasts)[0].type).toBe('error');
	});

	it('removes message after duration', () => {
		toast('Temporary', 'info', 1000);
		expect(get(toasts)).toHaveLength(1);
		vi.advanceTimersByTime(1000);
		expect(get(toasts)).toHaveLength(0);
	});

	it('does not remove message before duration elapses', () => {
		toast('Sticky', 'info', 3000);
		vi.advanceTimersByTime(2999);
		expect(get(toasts)).toHaveLength(1);
	});

	it('can stack multiple toasts', () => {
		toast('A', 'success');
		toast('B', 'error');
		toast('C', 'info');
		expect(get(toasts)).toHaveLength(3);
	});

	it('removes only the expired toast when multiple exist', () => {
		toast('Short', 'info', 500);
		toast('Long', 'info', 5000);
		vi.advanceTimersByTime(500);
		const remaining = get(toasts);
		expect(remaining).toHaveLength(1);
		expect(remaining[0].text).toBe('Long');
	});
});

describe('toastWithUndo', () => {
	it('adds a toast with undoEventId', () => {
		toastWithUndo('Item deleted', 'event-abc-123');
		const msgs = get(toasts);
		expect(msgs).toHaveLength(1);
		expect(msgs[0]).toMatchObject({
			text: 'Item deleted',
			type: 'success',
			undoEventId: 'event-abc-123'
		});
	});

	it('removes after default duration (5s)', () => {
		toastWithUndo('Deleted', 'evt-1');
		expect(get(toasts)).toHaveLength(1);
		vi.advanceTimersByTime(5000);
		expect(get(toasts)).toHaveLength(0);
	});
});

describe('dismissToast', () => {
	it('removes a specific toast by id', () => {
		toast('Keep', 'info');
		toast('Remove', 'error');
		const id = get(toasts).find((m) => m.text === 'Remove')!.id;
		dismissToast(id);
		const remaining = get(toasts);
		expect(remaining).toHaveLength(1);
		expect(remaining[0].text).toBe('Keep');
	});

	it('is a no-op for unknown id', () => {
		toast('One', 'info');
		dismissToast(99999);
		expect(get(toasts)).toHaveLength(1);
	});
});
