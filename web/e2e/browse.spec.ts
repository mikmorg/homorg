import { test, expect, ROOT_ID } from './fixtures';

test.describe('browse — container hierarchy', () => {
	test('API-created container appears at root and is clickable', async ({ page, api }) => {
		const name = `Garage ${Date.now()}`;
		const id = await api.createContainer(ROOT_ID, name);

		await page.goto(`/browse?id=${ROOT_ID}`);
		const row = page.getByRole('button', { name: new RegExp(name) });
		await expect(row).toBeVisible();

		await row.click();
		await page.waitForURL(`**/browse?id=${id}`);
		await expect(page.getByRole('link', { name })).toBeVisible();
		await expect(page.getByText('This container is empty')).toBeVisible();
	});

	test('nested container shows under its parent and back-nav returns to parent', async ({
		page,
		api
	}) => {
		const parentName = `Shed ${Date.now()}`;
		const childName = `Toolbox ${Date.now()}`;
		const parentId = await api.createContainer(ROOT_ID, parentName);
		await api.createContainer(parentId, childName);

		await page.goto(`/browse?id=${parentId}`);
		await expect(page.getByRole('button', { name: new RegExp(childName) })).toBeVisible();

		await page.getByRole('button', { name: 'Back' }).click();
		await page.waitForURL(`**/browse?id=${ROOT_ID}`);
		await expect(page.getByRole('button', { name: new RegExp(parentName) })).toBeVisible();
	});
});
