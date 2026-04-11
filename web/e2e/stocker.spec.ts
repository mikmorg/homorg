import { test, expect, ROOT_ID } from './fixtures';

test.describe('stocker — session lifecycle', () => {
	test('start session from empty state and end it', async ({ page }) => {
		await page.goto('/stocker');

		await expect(page.getByText('No sessions yet')).toBeVisible();
		await page.getByRole('button', { name: 'Start your first session' }).click();

		await page.waitForURL(/\/stocker\/[0-9a-f-]{36}/);
		await expect(page.getByText('Scanned', { exact: true })).toBeVisible();

		await page.getByRole('button', { name: 'End' }).click();

		await page.waitForURL('**/stocker');
		await expect(page.getByText('Session', { exact: true })).toBeVisible();
		await expect(page.getByText('live', { exact: true })).toHaveCount(0);
	});

	test('new session via header button appears as live in the list', async ({ page }) => {
		await page.goto('/stocker');
		await page.getByRole('button', { name: 'New session' }).click();
		await page.getByPlaceholder('Session notes (optional)').fill('e2e-header-button');
		await page.getByRole('button', { name: 'Start session' }).click();

		await page.waitForURL(/\/stocker\/[0-9a-f-]{36}/);
		await page.goto('/stocker');

		await expect(page.getByText('Active session')).toBeVisible();
		await expect(page.getByText('e2e-header-button')).toBeVisible();
		await expect(page.getByText('live', { exact: true })).toBeVisible();
	});
});

test.describe('stocker — scan log', () => {
	test('API-submitted create_and_place appears in the session scan log', async ({
		page,
		api
	}) => {
		const containerName = `Scan Bin ${Date.now()}`;
		const itemName = `Scan Widget ${Date.now()}`;
		const barcode = `HOM-E2E-${Date.now()}`;

		const containerId = await api.createContainer(ROOT_ID, containerName);
		const sessionId = await api.startSession(containerId);

		await api.submitBatch(sessionId, [
			{
				type: 'create_and_place',
				barcode,
				name: itemName,
				scanned_at: new Date().toISOString()
			}
		]);

		await page.goto(`/stocker/${sessionId}`);

		await expect(
			page.getByText(`Created: ${itemName} → ${containerName}`)
		).toBeVisible({ timeout: 5_000 });
	});

	test('SSE stream delivers live scan events after page load', async ({ page, api }) => {
		const containerName = `SSE Bin ${Date.now()}`;
		const itemName = `SSE Widget ${Date.now()}`;
		const barcode = `HOM-SSE-${Date.now()}`;

		const containerId = await api.createContainer(ROOT_ID, containerName);
		const sessionId = await api.startSession(containerId);

		await page.goto(`/stocker/${sessionId}`);
		await expect(page.getByText('Waiting for scans…')).toBeVisible();

		await api.submitBatch(sessionId, [
			{
				type: 'create_and_place',
				barcode,
				name: itemName,
				scanned_at: new Date().toISOString()
			}
		]);

		await expect(
			page.getByText(`Created: ${itemName} → ${containerName}`)
		).toBeVisible({ timeout: 10_000 });
	});
});
