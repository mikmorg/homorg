import { test, expect } from './fixtures';

test.describe('error paths', () => {
	test('navigating to nonexistent item shows error state', async ({ page }) => {
		await page.goto('/browse?id=00000000-0000-0000-0000-999999999999');
		// Should show some indication the item wasn't found
		await expect(
			page.getByText(/not found|doesn't exist|error/i)
		).toBeVisible({ timeout: 10_000 });
	});

	test('unauthenticated API call returns 401', async ({ playwright }) => {
		// Verify the backend rejects requests without a valid token
		const ctx = await playwright.request.newContext({
			baseURL: 'http://localhost:8080/api/v1/'
		});
		const res = await ctx.get('containers/00000000-0000-0000-0000-000000000001/children');
		expect(res.status()).toBe(401);
		await ctx.dispose();
	});
});
