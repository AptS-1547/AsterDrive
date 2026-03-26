import path from "node:path";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import { VitePWA } from "vite-plugin-pwa";

export default defineConfig(({ command }) => {
	const isDevServer = command === "serve";

	return {
		plugins: [
			react(),
			tailwindcss(),
			VitePWA({
				registerType: "prompt",
				includeAssets: ["favicon.svg"],
				devOptions: {
					enabled: true,
					navigateFallbackAllowlist: [/^\/$/],
				},
				manifest: {
					name: "AsterDrive",
					short_name: "AsterDrive",
					description: "Self-hosted cloud storage",
					theme_color: "#0F172A",
					background_color: "#ffffff",
					display: "standalone",
					icons: [
						{
							src: "/favicon.svg",
							sizes: "any",
							type: "image/svg+xml",
							purpose: "any",
						},
						{
							src: "/favicon.svg",
							sizes: "any",
							type: "image/svg+xml",
							purpose: "maskable",
						},
					],
				},
				workbox: {
					globPatterns: isDevServer
						? []
						: ["**/*.{html,js,css,ico,png,svg,woff2,mjs}"],
					navigateFallback: "index.html",
					navigateFallbackDenylist: [/^\/api\//, /^\/health\//],
					runtimeCaching: [
						{
							urlPattern: ({ request, url }) =>
								url.pathname.startsWith("/assets/") &&
								(request.destination === "script" ||
									request.destination === "style"),
							handler: "StaleWhileRevalidate",
							options: {
								cacheName: "asset-chunks",
								expiration: {
									maxEntries: 128,
									maxAgeSeconds: 60 * 60 * 24 * 30,
								},
							},
						},
					],
				},
			}),
		],
		base: "/",
		resolve: {
			alias: {
				"@": path.resolve(__dirname, "./src"),
			},
		},
		server: {
			proxy: {
				"/api": "http://127.0.0.1:3000",
				"/health": "http://127.0.0.1:3000",
			},
		},
		build: {
			target: "esnext",
			outDir: "dist",
			emptyOutDir: true,
			rollupOptions: {
				output: {
					manualChunks(id) {
						if (!id.includes("node_modules")) return;
						if (
							id.includes("/react-dom/") ||
							id.includes("/react/") ||
							id.includes("/scheduler/")
						)
							return "vendor-react";
						if (id.includes("/react-router")) return "vendor-router";
						if (id.includes("/@base-ui/")) return "vendor-ui";
						if (id.includes("/i18next") || id.includes("/react-i18next/"))
							return "vendor-i18n";
						if (id.includes("/react-icons/")) return "vendor-icons";
					},
				},
			},
		},
	};
});
