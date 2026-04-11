import { test, expect } from '@playwright/test';

test.use({ storageState: { cookies: [], origins: [] } });

test('unauthenticated root redirects to login', async ({ page }) => {
	await page.goto('/');
	await page.waitForURL('**/login', { timeout: 10_000 });

	await expect(page.getByRole('heading', { name: 'Homorg' })).toBeVisible();
	await expect(page.getByLabel('Username')).toBeVisible();
	await expect(page.getByLabel('Password', { exact: true })).toBeVisible();
});

test('authenticated root redirects to browse', async ({ browser }) => {
	const context = await browser.newContext({ storageState: 'playwright/.auth/admin.json' });
	const page = await context.newPage();
	await page.goto('/');
	await page.waitForURL('**/browse', { timeout: 10_000 });
	await context.close();
});
