import { execSync } from 'node:child_process';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = fileURLToPath(new URL('.', import.meta.url));
const BACKEND_HEALTH = 'http://localhost:8080/api/v1/health';

export default async function globalSetup() {
	try {
		const res = await fetch(BACKEND_HEALTH);
		if (!res.ok) throw new Error(`health returned ${res.status}`);
	} catch (err) {
		const msg = err instanceof Error ? err.message : String(err);
		throw new Error(
			`Backend not reachable at ${BACKEND_HEALTH}. Start it with 'cargo run' before running e2e tests.\n  ${msg}`
		);
	}

	// Warm the Vite dev server so the first spec doesn't eat the cold-compile
	// latency inside its 5s expect timeout. Playwright's `webServer` config
	// waits for the root URL, but route-level compilation happens on demand.
	try {
		await fetch('http://localhost:5173/setup');
	} catch {
		// Dev server may not be up yet in some contexts; Playwright's webServer
		// will still block until ready. Ignore.
	}

	const script = resolve(here, '..', '..', 'scripts', 'reset-db.sh');
	execSync(script, { stdio: 'inherit' });
}
