import { test as setup, expect } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { dirname } from 'node:path';
import { ADMIN_USER, ADMIN_PASS, STORAGE_STATE } from './constants';

setup('create admin account via setup UI', async ({ page }) => {
	mkdirSync(dirname(STORAGE_STATE), { recursive: true });

	await page.goto('/setup');
	await expect(page.getByRole('heading', { name: 'Welcome to Homorg' })).toBeVisible();

	await page.getByLabel('Admin username').fill(ADMIN_USER);
	await page.getByLabel('Password', { exact: true }).fill(ADMIN_PASS);
	await page.getByLabel('Confirm password').fill(ADMIN_PASS);
	await page.getByRole('button', { name: 'Create account' }).click();

	await page.waitForURL('**/browse');

	await page.context().storageState({ path: STORAGE_STATE });
});
