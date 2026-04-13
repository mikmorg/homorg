import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import ConfirmDialog from './ConfirmDialog.svelte';

// The dialog's backdrop has aria-hidden="true", so we need { hidden: true }
// to find elements inside it with getByRole/queryByRole.
const HIDDEN = { hidden: true };

describe('ConfirmDialog', () => {
	it('does not render dialog when open is false', () => {
		render(ConfirmDialog, {
			props: { open: false, onConfirm: vi.fn() }
		});
		expect(screen.queryByRole('dialog', HIDDEN)).toBeNull();
	});

	it('renders dialog when open is true', () => {
		render(ConfirmDialog, {
			props: { open: true, onConfirm: vi.fn() }
		});
		expect(screen.getByRole('dialog', HIDDEN)).toBeTruthy();
		expect(screen.getByText('Are you sure?')).toBeTruthy();
	});

	it('renders custom title and message', () => {
		render(ConfirmDialog, {
			props: {
				open: true,
				title: 'Delete item?',
				message: 'This cannot be undone.',
				onConfirm: vi.fn()
			}
		});
		expect(screen.getByText('Delete item?')).toBeTruthy();
		expect(screen.getByText('This cannot be undone.')).toBeTruthy();
	});

	it('does not render message paragraph when message is empty', () => {
		render(ConfirmDialog, {
			props: { open: true, message: '', onConfirm: vi.fn() }
		});
		const dialog = screen.getByRole('dialog', HIDDEN);
		expect(dialog.querySelectorAll('p').length).toBe(0);
	});

	it('calls onConfirm when confirm button clicked', async () => {
		const onConfirm = vi.fn();
		render(ConfirmDialog, {
			props: { open: true, onConfirm }
		});
		await fireEvent.click(screen.getByText('Confirm'));
		expect(onConfirm).toHaveBeenCalledOnce();
	});

	it('calls onCancel when cancel button clicked', async () => {
		const onCancel = vi.fn();
		render(ConfirmDialog, {
			props: { open: true, onConfirm: vi.fn(), onCancel }
		});
		await fireEvent.click(screen.getByText('Cancel'));
		expect(onCancel).toHaveBeenCalledOnce();
	});

	it('uses custom button labels', () => {
		render(ConfirmDialog, {
			props: {
				open: true,
				confirmLabel: 'Delete',
				cancelLabel: 'Keep',
				onConfirm: vi.fn()
			}
		});
		expect(screen.getByText('Delete')).toBeTruthy();
		expect(screen.getByText('Keep')).toBeTruthy();
	});

	it('applies destructive styling when destructive is true', () => {
		render(ConfirmDialog, {
			props: { open: true, destructive: true, onConfirm: vi.fn() }
		});
		const buttons = screen.getAllByRole('button', HIDDEN);
		const confirmBtn = buttons.find((b) => b.className.includes('btn-danger'));
		expect(confirmBtn).toBeTruthy();
	});

	it('applies primary styling when destructive is false', () => {
		render(ConfirmDialog, {
			props: { open: true, destructive: false, onConfirm: vi.fn() }
		});
		const buttons = screen.getAllByRole('button', HIDDEN);
		const confirmBtn = buttons.find((b) => b.className.includes('btn-primary'));
		expect(confirmBtn).toBeTruthy();
	});

	it('disables buttons when loading', () => {
		render(ConfirmDialog, {
			props: { open: true, loading: true, onConfirm: vi.fn() }
		});
		const buttons = screen.getAllByRole('button', HIDDEN);
		for (const btn of buttons) {
			expect(btn.hasAttribute('disabled')).toBe(true);
		}
	});

	it('shows spinner instead of label when loading', () => {
		render(ConfirmDialog, {
			props: { open: true, loading: true, onConfirm: vi.fn() }
		});
		const dialog = screen.getByRole('dialog', HIDDEN);
		expect(dialog.querySelector('.animate-spin')).toBeTruthy();
		// Confirm label should not be visible
		expect(screen.queryByText('Confirm')).toBeNull();
	});
});
