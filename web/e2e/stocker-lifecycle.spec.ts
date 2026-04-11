import { test, expect } from './fixtures';

test.describe('stocker — session lifecycle', () => {
	test('start session from empty state and end it', async ({ page }) => {
		await page.goto('/stocker');

		await expect(page.getByText('No sessions yet')).toBeVisible();
		await page.getByRole('button', { name: 'Start your first session' }).click();

		await page.waitForURL(/\/stocker\/[0-9a-f-]{36}/);
		await expect(page.getByText('Scanned', { exact: true })).toBeVisible();

		await page.getByRole('button', { name: 'End' }).click();

		await page.waitForURL('**/stocker');
		await expect(page.getByText('Session', { exact: true })).toBeVisible();
		await expect(page.getByText('live', { exact: true })).toHaveCount(0);
	});

	test('new session via header button appears as live in the list', async ({ page }) => {
		await page.goto('/stocker');
		await page.getByRole('button', { name: 'New session' }).click();
		await page.getByPlaceholder('Session notes (optional)').fill('e2e-header-button');
		await page.getByRole('button', { name: 'Start session' }).click();

		await page.waitForURL(/\/stocker\/[0-9a-f-]{36}/);
		await page.goto('/stocker');

		await expect(page.getByText('Active session')).toBeVisible();
		await expect(page.getByText('e2e-header-button')).toBeVisible();
		await expect(page.getByText('live', { exact: true })).toBeVisible();
	});
});
