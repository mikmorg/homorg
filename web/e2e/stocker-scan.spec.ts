import { test, expect, ROOT_ID } from './fixtures';

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
