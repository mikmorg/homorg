import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vitest/config';
import { VitePWA } from 'vite-plugin-pwa';
import tailwindcss from '@tailwindcss/vite';
import fs from 'node:fs';
import path from 'node:path';

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
		https: fs.existsSync(path.resolve(__dirname, '.certs/localhost+2.pem'))
			? {
					cert: fs.readFileSync(path.resolve(__dirname, '.certs/localhost+2.pem')),
					key: fs.readFileSync(path.resolve(__dirname, '.certs/localhost+2-key.pem'))
				}
			: undefined,
		proxy: {
			'/api': { target: 'http://localhost:8080', changeOrigin: true },
			'/files': { target: 'http://localhost:8080', changeOrigin: true }
		}
	},
	build: {
		target: 'es2022'
	},
	resolve: {
		// Svelte 5 component tests need browser conditions to avoid server-only mount()
		conditions: ['browser']
	},
	test: {
		include: ['src/**/*.test.ts'],
		environment: 'jsdom',
		globals: true
	}
});
