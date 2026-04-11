import { test, expect } from './fixtures';

test.describe('admin — categories CRUD', () => {
	test('create, edit, and delete a category via the admin UI', async ({ page }) => {
		const name = `E2E Cat ${Date.now()}`;
		const renamed = `${name} (renamed)`;

		await page.goto('/admin/categories');

		await page.getByRole('button', { name: 'Add' }).click();
		await page.getByLabel('Name *').fill(name);
		await page.getByLabel('Description').fill('created by e2e');
		await page.getByRole('button', { name: 'Create', exact: true }).click();

		const row = page.getByTestId('category-row').filter({ hasText: name });
		await expect(row).toBeVisible();
		await expect(page.getByText('Category created')).toBeVisible();

		await row.getByRole('button', { name: 'Edit' }).click();
		await page.getByLabel('Name *').fill(renamed);
		await page.getByRole('button', { name: 'Update' }).click();

		const renamedRow = page.getByTestId('category-row').filter({ hasText: renamed });
		await expect(renamedRow).toBeVisible();
		await expect(page.getByText('Category updated')).toBeVisible();

		page.once('dialog', (d) => d.accept());
		await renamedRow.getByRole('button', { name: 'Delete' }).click();

		await expect(page.getByTestId('category-row').filter({ hasText: renamed })).toHaveCount(0);
		await expect(page.getByText('Category deleted')).toBeVisible();
	});
});
