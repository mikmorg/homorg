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

	const script = resolve(here, '..', '..', 'scripts', 'reset-db.sh');
	execSync(script, { stdio: 'inherit' });
}
