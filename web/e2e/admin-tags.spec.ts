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

		// Rename
		const row = page.locator('li').filter({ hasText: name });
		await row.getByRole('button', { name: 'Rename' }).click();
		const input = row.getByRole('textbox');
		await input.fill(renamed);
		await row.getByRole('button', { name: 'Save' }).click();
		await expect(page.getByText(renamed)).toBeVisible();
		await expect(page.getByText('Tag renamed')).toBeVisible();

		// Delete
		const renamedRow = page.locator('li').filter({ hasText: renamed });
		page.once('dialog', (d) => d.accept());
		await renamedRow.getByRole('button', { name: 'Delete' }).click();
		await expect(page.locator('li').filter({ hasText: renamed })).toHaveCount(0);
		await expect(page.getByText('Tag deleted')).toBeVisible();
	});
});
