import { render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "@/App";

const mockState = vi.hoisted(() => ({
	authStore: {
		bootOffline: false,
		checkAuth: vi.fn(),
		isAuthenticated: false,
		isChecking: false,
		user: null as { role?: string } | null,
	},
	brandingLoad: vi.fn(),
	setAuthState: vi.fn(),
	themeInit: vi.fn(),
}));

vi.mock("react-router-dom", () => ({
	RouterProvider: () => <div data-testid="router-provider" />,
}));

vi.mock("sonner", () => ({
	Toaster: () => <div data-testid="toaster" />,
}));

vi.mock("@/router", () => ({
	router: {},
}));

vi.mock("@/hooks/usePwaUpdate", () => ({
	usePwaUpdate: vi.fn(),
}));

vi.mock("@/hooks/useStorageChangeEvents", () => ({
	useStorageChangeEvents: vi.fn(),
}));

vi.mock("@/components/layout/OfflineBootFallback", () => ({
	OfflineBootFallback: () => <div data-testid="offline-fallback" />,
}));

vi.mock("@/stores/brandingStore", () => ({
	useBrandingStore: {
		getState: () => ({
			load: mockState.brandingLoad,
		}),
	},
}));

vi.mock("@/stores/themeStore", () => ({
	useThemeStore: {
		getState: () => ({
			init: mockState.themeInit,
		}),
	},
}));

vi.mock("@/stores/authStore", () => {
	const useAuthStore = Object.assign(
		(selector: (state: typeof mockState.authStore) => unknown) =>
			selector(mockState.authStore),
		{
			setState: (...args: unknown[]) => mockState.setAuthState(...args),
		},
	);

	return {
		useAuthStore,
	};
});

describe("App", () => {
	beforeEach(() => {
		mockState.authStore.bootOffline = false;
		mockState.authStore.checkAuth.mockReset();
		mockState.authStore.isAuthenticated = false;
		mockState.authStore.isChecking = false;
		mockState.authStore.user = null;
		mockState.brandingLoad.mockReset();
		mockState.setAuthState.mockReset();
		mockState.themeInit.mockReset();
	});

	afterEach(() => {
		window.history.replaceState({}, "", "/");
	});

	it("skips the bootstrap auth check on login", () => {
		window.history.replaceState({}, "", "/login");

		render(<App />);

		expect(mockState.authStore.checkAuth).not.toHaveBeenCalled();
		expect(mockState.setAuthState).toHaveBeenCalledWith({ isChecking: false });
	});

	it("runs the bootstrap auth check on protected routes", () => {
		window.history.replaceState({}, "", "/");

		render(<App />);

		expect(mockState.authStore.checkAuth).toHaveBeenCalledTimes(1);
		expect(mockState.setAuthState).not.toHaveBeenCalled();
	});
});
