import { test, expect } from './fixtures';

test.describe('admin — container types CRUD', () => {
	test('create, edit, and delete a container type via the admin UI', async ({ page }) => {
		const name = `E2E Type ${Date.now()}`;
		const renamed = `${name} (edited)`;

		await page.goto('/admin/container-types');

		// Create
		await page.getByRole('button', { name: 'Add' }).click();
		await expect(page.getByRole('dialog')).toBeVisible();
		await page.getByLabel('Name *').fill(name);
		await page.getByLabel('Description').fill('created by e2e');
		await page.getByRole('button', { name: 'Create', exact: true }).click();

		await expect(page.getByText(name)).toBeVisible();

		// Edit
		const row = page.locator('li, tr, div').filter({ hasText: name });
		await row.getByRole('button', { name: 'Edit' }).click();
		await expect(page.getByRole('dialog')).toBeVisible();
		await page.getByLabel('Name *').fill(renamed);
		await page.getByRole('button', { name: 'Update' }).click();

		await expect(page.getByText(renamed)).toBeVisible();

		// Delete
		const editedRow = page.locator('li, tr, div').filter({ hasText: renamed });
		page.once('dialog', (d) => d.accept());
		await editedRow.getByRole('button', { name: 'Delete' }).click();
		await expect(page.getByText(renamed)).toHaveCount(0);
	});
});
