import { execSync } from 'node:child_process';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = fileURLToPath(new URL('.', import.meta.url));

export default async function globalSetup() {
	const script = resolve(here, '..', '..', 'scripts', 'reset-db.sh');
	execSync(script, { stdio: 'inherit' });
}
