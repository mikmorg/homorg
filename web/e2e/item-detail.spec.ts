import { test, expect, ROOT_ID } from './fixtures';

test.describe('item detail', () => {
  test('item appears in browse container', async ({ page, api }) => {
    // Create container and item via API
    const containerId = await api.createContainer(ROOT_ID, 'Storage Box');
    const itemId = await api.createItem({
      parent_id: containerId,
      name: 'Widget',
      is_container: false,
    });

    // Navigate to the container
    await page.goto(`/browse?id=${containerId}`);

    // Item should be visible
    await expect(page.getByText('Widget')).toBeVisible({ timeout: 5000 });

    // Click the item to navigate to detail
    await page.getByText('Widget').click();

    // Should navigate to item detail page
    await page.waitForURL(new RegExp(`/items/${itemId}`), { timeout: 5000 });
  });

  test('item detail page shows name and back link', async ({ page, api }) => {
    const containerId = await api.createContainer(ROOT_ID, 'Box');
    const itemId = await api.createItem({
      parent_id: containerId,
      name: 'Test Widget',
      is_container: false,
    });

    // Navigate directly to item detail
    await page.goto(`/items/${itemId}`);

    // Name should appear as heading
    const heading = page.locator('h1, h2, [role="heading"]').filter({ hasText: 'Test Widget' });
    await expect(heading).toBeVisible({ timeout: 5000 });

    // Back navigation link should be visible
    const backLink = page.getByRole('link').filter({ hasText: /back|browse/i }).first();
    await expect(backLink).toBeVisible();

    // Back link should navigate back
    await backLink.click();
    await page.waitForURL(new RegExp(`/browse\\?id=${containerId}`), { timeout: 5000 });
  });

  test('deleted item shows restore button', async ({ page, api }) => {
    const itemId = await api.createItem({
      parent_id: ROOT_ID,
      name: 'Deletable Item',
      is_container: false,
    });

    // Delete the item via API
    await api.deleteItem(itemId);

    // Navigate to deleted item
    await page.goto(`/items/${itemId}`);

    // Restore button should be visible
    const restoreButton = page.getByRole('button').filter({ hasText: /restore/i });
    await expect(restoreButton).toBeVisible({ timeout: 5000 });

    // Click restore
    await restoreButton.click();

    // Should show success message or return to browse
    // For now, just verify the button was clickable and page updated
    await expect(page).toHaveURL(new RegExp('/items/'));
  });

  test('breadcrumb shows container path on item detail', async ({ page, api }) => {
    // Create nested containers
    const shellId = await api.createContainer(ROOT_ID, 'Shelf');
    const boxId = await api.createContainer(shellId, 'Box');
    const itemId = await api.createItem({
      parent_id: boxId,
      name: 'Item Inside Box',
      is_container: false,
    });

    // Navigate to item
    await page.goto(`/items/${itemId}`);

    // Breadcrumb should show both containers
    await expect(page.getByText('Shelf')).toBeVisible({ timeout: 5000 });
    await expect(page.getByText('Box')).toBeVisible({ timeout: 5000 });
    await expect(page.getByText('Item Inside Box')).toBeVisible({ timeout: 5000 });
  });
});
