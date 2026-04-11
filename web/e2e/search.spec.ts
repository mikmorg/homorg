import { test, expect, ROOT_ID } from './fixtures';

test.describe('search', () => {
	test('text query returns matching container and click navigates to browse', async ({
		page,
		api
	}) => {
		const unique = `Zxq${Date.now()}`;
		const name = `${unique} Cabinet`;
		const id = await api.createContainer(ROOT_ID, name);

		await page.goto('/search');
		await page.getByPlaceholder('Search items and containers…').fill(unique);

		const row = page.getByText(name);
		await expect(row).toBeVisible({ timeout: 5_000 });

		await row.click();
		await page.waitForURL(`**/browse?id=${id}`);
	});

	test('query with no matches shows empty state', async ({ page }) => {
		await page.goto('/search');
		await page
			.getByPlaceholder('Search items and containers…')
			.fill(`no-such-item-${Date.now()}`);

		await expect(page.getByText('No results')).toBeVisible({ timeout: 5_000 });
	});
});
