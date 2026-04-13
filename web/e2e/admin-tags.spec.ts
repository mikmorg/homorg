import { test, expect } from './fixtures';

test.describe('admin — tags CRUD', () => {
	test('create, rename, and delete a tag via the admin UI', async ({ page }) => {
		const name = `E2E Tag ${Date.now()}`;
		const renamed = `${name} (renamed)`;

		await page.goto('/admin/tags');

		// Create
		await page.getByPlaceholder('New tag name').fill(name);
		await page.getByRole('button', { name: 'Add' }).click();
		await expect(page.getByText(name)).toBeVisible();
		await expect(page.getByText('Tag created')).toBeVisible();

		// Rename — clicking opens inline edit mode which replaces the text
		// span with an input, so the hasText filter won't match after click.
		const row = page.locator('div.divide-y > div').filter({ hasText: name });
		await row.getByRole('button', { name: 'Rename' }).click();
		// After click, the row's text content changes — find the input and save directly
		const input = page.locator('div.divide-y > div').getByRole('textbox');
		await input.fill(renamed);
		await page.getByRole('button', { name: 'Save' }).click();
		await expect(page.getByText(renamed)).toBeVisible();
		await expect(page.getByText('Tag renamed')).toBeVisible();

		// Delete
		const renamedRow = page.locator('div.divide-y > div').filter({ hasText: renamed });
		page.once('dialog', (d) => d.accept());
		await renamedRow.getByRole('button', { name: 'Delete' }).click();
		await expect(page.locator('div.divide-y > div').filter({ hasText: renamed })).toHaveCount(0);
		await expect(page.getByText('Tag deleted')).toBeVisible();
	});
});
