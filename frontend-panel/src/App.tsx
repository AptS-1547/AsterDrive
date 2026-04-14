import { useEffect } from "react";
import { RouterProvider } from "react-router-dom";
import { Toaster } from "sonner";
import { OfflineBootFallback } from "@/components/layout/OfflineBootFallback";
import { usePwaUpdate } from "@/hooks/usePwaUpdate";
import { useStorageChangeEvents } from "@/hooks/useStorageChangeEvents";
import { router } from "@/router";
import { useAuthStore } from "@/stores/authStore";
import { useBrandingStore } from "@/stores/brandingStore";
import { usePreviewAppStore } from "@/stores/previewAppStore";
import { useThemeStore } from "@/stores/themeStore";

function shouldSkipInitialAuthCheck(pathname: string) {
	return pathname === "/login" || pathname.startsWith("/s/");
}

function App() {
	const checkAuth = useAuthStore((s) => s.checkAuth);
	const isChecking = useAuthStore((s) => s.isChecking);
	const isAuthenticated = useAuthStore((s) => s.isAuthenticated);
	const bootOffline = useAuthStore((s) => s.bootOffline);
	const userRole = useAuthStore((s) => s.user?.role);
	usePwaUpdate();
	useStorageChangeEvents();

	useEffect(() => {
		void useBrandingStore.getState().load();
		void usePreviewAppStore.getState().load();
		if (!shouldSkipInitialAuthCheck(window.location.pathname)) {
			checkAuth();
		} else {
			useAuthStore.setState({ isChecking: false });
		}
		useThemeStore.getState().init();
	}, [checkAuth]);

	useEffect(() => {
		if (isChecking || !isAuthenticated) return;

		let cancelled = false;

		void import("@/lib/pwaWarmup").then(({ warmupRouteChunks }) => {
			if (cancelled) return;
			warmupRouteChunks(userRole === "admin" ? "admin" : "user");
		});

		return () => {
			cancelled = true;
		};
	}, [isAuthenticated, isChecking, userRole]);

	return (
		<>
			{bootOffline ? (
				<OfflineBootFallback />
			) : (
				<RouterProvider router={router} />
			)}
			<Toaster position="bottom-right" richColors swipeDirections={["right"]} />
		</>
	);
}

export default App;
