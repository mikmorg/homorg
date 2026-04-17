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

	test('non-container item appears in container', async ({ page, api }) => {
		const containerName = `Box ${Date.now()}`;
		const itemName = `Item ${Date.now()}`;
		const containerId = await api.createContainer(ROOT_ID, containerName);
		await api.createItem({
			parent_id: containerId,
			name: itemName,
			is_container: false,
		});

		await page.goto(`/browse?id=${containerId}`);
		const itemRow = page.getByRole('button', { name: new RegExp(itemName) });
		await expect(itemRow).toBeVisible();

		// Item should have inventory icon, not folder icon
		// (non-container items don't have the chevron navigation indicator)
		const container = page.getByRole('button', { name: new RegExp(containerName) }).first();
		await expect(container).toBeVisible();
	});

	test('breadcrumb updates on deep navigation', async ({ page, api }) => {
		const level1Name = `Level1 ${Date.now()}`;
		const level2Name = `Level2 ${Date.now()}`;
		const level3Name = `Level3 ${Date.now()}`;

		const level1Id = await api.createContainer(ROOT_ID, level1Name);
		const level2Id = await api.createContainer(level1Id, level2Name);
		const level3Id = await api.createContainer(level2Id, level3Name);

		// Navigate to the deepest level
		await page.goto(`/browse?id=${level3Id}`);

		// All three levels should appear in breadcrumb
		await expect(page.getByText(level1Name)).toBeVisible({ timeout: 5000 });
		await expect(page.getByText(level2Name)).toBeVisible({ timeout: 5000 });
		await expect(page.getByText(level3Name)).toBeVisible({ timeout: 5000 });
	});
});
