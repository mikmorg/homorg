import { test, expect, ROOT_ID } from './fixtures';

test.describe('journal — event feed', () => {
	test('API-created item appears as a Created event and is undoable', async ({ page, api }) => {
		const name = `Journal Widget ${Date.now()}`;
		await api.createContainer(ROOT_ID, name);

		await page.goto('/journal');

		const row = page.locator('div.group', { hasText: name }).first();
		await expect(row).toBeVisible();
		await expect(row.getByText('Created', { exact: true })).toBeVisible();

		await row.hover();
		await row.getByRole('button', { name: 'Undo' }).click();

		await expect(page.getByText('Event undone')).toBeVisible({ timeout: 5_000 });
	});

	test('filter dropdown restricts the feed to the selected event type', async ({ page, api }) => {
		const name = `Journal Filtered ${Date.now()}`;
		await api.createContainer(ROOT_ID, name);

		await page.goto('/journal');
		await expect(page.locator('div.group', { hasText: name }).first()).toBeVisible();

		await page.getByRole('combobox').selectOption('ItemDeleted');

		await expect(page.locator('div.group', { hasText: name })).toHaveCount(0);

		await page.getByRole('combobox').selectOption('');
		await expect(page.locator('div.group', { hasText: name }).first()).toBeVisible();
	});
});
