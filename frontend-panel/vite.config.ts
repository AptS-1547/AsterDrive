import path from "node:path";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import { VitePWA } from "vite-plugin-pwa";

function getNodeModulePackageName(id: string) {
	const normalizedId = id.replaceAll("\\", "/");
	const nodeModulesSegment = "/node_modules/";
	const nodeModulesIndex = normalizedId.lastIndexOf(nodeModulesSegment);

	if (nodeModulesIndex === -1) return null;

	const modulePath = normalizedId.slice(
		nodeModulesIndex + nodeModulesSegment.length,
	);
	const [scopeOrName, maybeName] = modulePath.split("/");

	if (!scopeOrName) return null;
	if (scopeOrName.startsWith("@")) {
		return maybeName ? `${scopeOrName}/${maybeName}` : null;
	}

	return scopeOrName;
}

export default defineConfig(({ command }) => {
	const isDevServer = command === "serve";
	const rootReactPath = path.resolve(__dirname, "./node_modules/react");
	const rootReactDomPath = path.resolve(__dirname, "./node_modules/react-dom");

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
						: ["**/*.{html,js,css,ico,png,svg,woff2,ttf,mjs,bcmap}"],
					globIgnores: isDevServer
						? []
						: ["assets/vendor-editor-*.js", "assets/ts.worker-*.js"],
					navigateFallback: "index.html",
					navigateFallbackDenylist: [/^\/api\//, /^\/health\//],
					runtimeCaching: [
						{
							urlPattern: ({ request, url }) =>
								url.pathname.startsWith("/assets/") &&
								(request.destination === "script" ||
									request.destination === "style" ||
									request.destination === "font" ||
									request.destination === "worker"),
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
				"react/jsx-dev-runtime": path.resolve(
					rootReactPath,
					"./jsx-dev-runtime.js",
				),
				"react/jsx-runtime": path.resolve(rootReactPath, "./jsx-runtime.js"),
				"react-dom": rootReactDomPath,
				react: rootReactPath,
			},
			dedupe: ["react", "react-dom"],
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
						const packageName = getNodeModulePackageName(id);
						if (!packageName) return;

						if (
							packageName === "react" ||
							packageName === "react-dom" ||
							packageName === "scheduler"
						) {
							return "vendor-react";
						}

						if (
							packageName === "react-router" ||
							packageName === "react-router-dom"
						) {
							return "vendor-router";
						}

						if (
							packageName === "@base-ui/react" ||
							packageName === "@floating-ui/react-dom"
						) {
							return "vendor-ui";
						}

						if (packageName === "i18next" || packageName === "react-i18next") {
							return "vendor-i18n";
						}

						if (
							packageName === "react-icons" ||
							packageName === "@devicon/react" ||
							packageName === "react-devicons"
						) {
							return "vendor-icons";
						}

						if (packageName === "monaco-editor") {
							return "vendor-editor";
						}

						if (packageName === "react-pdf" || packageName === "pdfjs-dist") {
							return "vendor-pdf";
						}

						if (
							packageName === "react-markdown" ||
							packageName === "remark-gfm" ||
							packageName === "rehype-sanitize" ||
							packageName === "prism-react-renderer" ||
							packageName === "papaparse" ||
							packageName === "xml-formatter"
						) {
							return "vendor-preview";
						}
					},
				},
			},
		},
		test: {
			environment: "jsdom",
			setupFiles: "./src/test/setup.ts",
			restoreMocks: true,
			server: {
				deps: {
					inline: [/^react-devicons(?:\/|$)/],
				},
			},
		},
	};
});
