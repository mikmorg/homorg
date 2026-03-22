import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vitest/config';
import { VitePWA } from 'vite-plugin-pwa';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
	plugins: [
		tailwindcss(),
		sveltekit(),
		VitePWA({
			registerType: 'prompt',
			includeAssets: ['favicon.ico', 'icons/*.png', 'icons/*.svg'],
			manifest: {
				name: 'Homorg',
				short_name: 'Homorg',
				description: 'High-velocity personal home inventory system',
				theme_color: '#0f172a',
				background_color: '#0f172a',
				display: 'standalone',
				start_url: '/',
				categories: ['utilities'],
				icons: [
					{ src: '/icons/icon-192.png', sizes: '192x192', type: 'image/png', purpose: 'any maskable' },
					{ src: '/icons/icon-512.png', sizes: '512x512', type: 'image/png', purpose: 'any maskable' }
				]
			},
			workbox: {
				globPatterns: ['**/*.{js,css,html,ico,png,svg,woff2}'],
				runtimeCaching: [
					{
						urlPattern: /^https?:\/\/.*\/api\/v1\/(categories|tags|container-types)/,
						handler: 'StaleWhileRevalidate',
						options: {
							cacheName: 'taxonomy-cache',
							expiration: { maxEntries: 50, maxAgeSeconds: 300 }
						}
					},
					{
						urlPattern: /^https?:\/\/.*\/files\//,
						handler: 'CacheFirst',
						options: {
							cacheName: 'images-cache',
							expiration: { maxEntries: 500, maxAgeSeconds: 86400 }
						}
					}
				]
			},
			devOptions: { enabled: true, type: 'module' }
		})
	],
	server: {
		proxy: {
			'/api': { target: 'http://localhost:8080', changeOrigin: true },
			'/files': { target: 'http://localhost:8080', changeOrigin: true }
		}
	},
	build: {
		target: 'es2022'
	},
	test: {
		include: ['src/**/*.test.ts'],
		environment: 'jsdom',
		globals: true
	}
});
