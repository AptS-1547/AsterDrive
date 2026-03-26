import { useEffect } from "react";
import { RouterProvider } from "react-router-dom";
import { Toaster } from "sonner";
import { usePwaUpdate } from "@/hooks/usePwaUpdate";
import { router } from "@/router";
import { useAuthStore } from "@/stores/authStore";
import { useThemeStore } from "@/stores/themeStore";

function shouldSkipInitialAuthCheck(pathname: string) {
	return pathname === "/login" || pathname.startsWith("/s/");
}

function App() {
	const checkAuth = useAuthStore((s) => s.checkAuth);
	usePwaUpdate();

	useEffect(() => {
		if (!shouldSkipInitialAuthCheck(window.location.pathname)) {
			checkAuth();
		} else {
			useAuthStore.setState({ isChecking: false });
		}
		useThemeStore.getState().init();
	}, [checkAuth]);

	return (
		<>
			<RouterProvider router={router} />
			<Toaster position="bottom-right" richColors swipeDirections={["right"]} />
		</>
	);
}

export default App;
