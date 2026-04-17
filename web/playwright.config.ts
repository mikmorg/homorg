import { defineConfig, devices } from '@playwright/test';

const STORAGE_STATE = 'playwright/.auth/admin.json';

export default defineConfig({
	testDir: './e2e',
	fullyParallel: false,
	forbidOnly: !!process.env.CI,
	retries: 0,
	workers: 1,
	reporter: [['list'], ['html', { open: 'never' }]],
	globalSetup: './e2e/global.setup.ts',

	use: {
		baseURL: 'https://localhost:5173',
		trace: 'retain-on-failure',
		screenshot: 'only-on-failure',
		video: 'retain-on-failure',
		ignoreHTTPSErrors: true
	},

	projects: [
		{
			name: 'setup',
			testMatch: /auth\.setup\.ts/,
			use: { ...devices['Desktop Chrome'] }
		},
		{
			name: 'chromium',
			testIgnore: /auth\.setup\.ts/,
			use: {
				...devices['Desktop Chrome'],
				storageState: STORAGE_STATE
			},
			dependencies: ['setup']
		}
	],

	// webServer config disabled: dev server is manually managed locally
	// For CI, uncomment webServer and ensure dev server supports HTTPS verification
	// webServer: {
	// 	command: 'npm run dev',
	// 	url: 'https://localhost:5173',
	// 	reuseExistingServer: true,
	// 	stdout: 'ignore',
	// 	stderr: 'pipe',
	// 	timeout: 60_000
	// }
});
