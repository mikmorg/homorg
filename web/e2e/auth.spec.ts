import { test, expect } from '@playwright/test';
import { ADMIN_USER, ADMIN_PASS } from './constants';

test.describe('login — unauthenticated', () => {
	test.use({ storageState: { cookies: [], origins: [] } });

	test('rejects bad credentials with visible error', async ({ page }) => {
		await page.goto('/login');
		await page.getByLabel('Username').fill(ADMIN_USER);
		await page.getByLabel('Password', { exact: true }).fill('wrong-password');
		await page.getByRole('button', { name: 'Sign in' }).click();

		await expect(page.getByText(/invalid|incorrect|failed|unauthor/i)).toBeVisible({
			timeout: 5_000
		});
		await expect(page).toHaveURL(/\/login$/);
	});

	test('accepts good credentials and redirects to browse', async ({ page }) => {
		await page.goto('/login');
		await page.getByLabel('Username').fill(ADMIN_USER);
		await page.getByLabel('Password', { exact: true }).fill(ADMIN_PASS);
		await page.getByRole('button', { name: 'Sign in' }).click();

		await page.waitForURL('**/browse');
	});
});

test.describe('session — authenticated', () => {
	test('persists across reload', async ({ page }) => {
		await page.goto('/browse');
		await expect(page).toHaveURL(/\/browse/);
		await page.reload();
		await expect(page).toHaveURL(/\/browse/);
	});

	test('logout clears session and returns to login', async ({ page }) => {
		await page.goto('/account');
		await page.getByRole('button', { name: 'Sign out' }).click();
		await page.waitForURL('**/login');

		await page.goto('/browse');
		await page.waitForURL('**/login');
	});
});
