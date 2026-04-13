import { test, expect } from './fixtures';

test.describe('admin — user management', () => {
	test('admin can view user list and see own account', async ({ page }) => {
		await page.goto('/admin/users');
		// The seeded admin user should appear in the list
		await expect(page.getByText('admin')).toBeVisible();
	});

	test('admin can generate invite code', async ({ page }) => {
		await page.goto('/admin/users');
		await page.getByRole('button', { name: 'Invite' }).click();
		// Invite code banner should appear with a Copy button
		await expect(page.getByRole('button', { name: 'Copy' })).toBeVisible();
	});
});
