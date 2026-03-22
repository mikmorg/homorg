import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: vitePreprocess(),
	kit: {
		adapter: adapter({
			fallback: 'index.html'
		}),
		alias: {
			$api: 'src/lib/api',
			$stores: 'src/lib/stores',
			$scanner: 'src/lib/scanner',
			$audio: 'src/lib/audio',
			$offline: 'src/lib/offline',
			$components: 'src/lib/components'
		}
	}
};

export default config;
