import { test, expect } from './fixtures';

test.describe('error paths', () => {
	test('navigating to nonexistent item shows error state', async ({ page }) => {
		await page.goto('/browse?id=00000000-0000-0000-0000-999999999999');
		// Should show some indication the item wasn't found
		await expect(
			page.getByText(/not found|doesn't exist|error/i)
		).toBeVisible({ timeout: 10_000 });
	});

	test('unauthenticated user is redirected to login', async ({ browser }) => {
		// Use a fresh context with no stored auth
		const ctx = await browser.newContext();
		const page = await ctx.newPage();
		await page.goto('/browse');
		// Should redirect to login
		await page.waitForURL('**/login', { timeout: 10_000 });
		await expect(page.getByRole('button', { name: /log in|sign in/i })).toBeVisible();
		await ctx.close();
	});
});
