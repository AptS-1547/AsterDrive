import { describe, expect, it, vi } from "vitest";

const createBrowserRouterMock = vi.fn((routes: unknown) => ({ routes }));

vi.mock("@/pages/ErrorPage", () => ({
	default: () => null,
}));

vi.mock("react-router-dom", async () => {
	const actual =
		await vi.importActual<typeof import("react-router-dom")>(
			"react-router-dom",
		);

	return {
		...actual,
		createBrowserRouter: createBrowserRouterMock,
	};
});

describe("router", () => {
	it("redirects unmatched routes to the home route", async () => {
		await import("./index");

		const routes = createBrowserRouterMock.mock.calls[0]?.[0] as Array<{
			element?: {
				props?: {
					replace?: boolean;
					to?: string;
				};
			};
			path?: string;
		}>;
		const fallbackRoute = routes.at(-1);

		expect(fallbackRoute?.path).toBe("*");
		expect(fallbackRoute?.element?.props?.to).toBe("/");
		expect(fallbackRoute?.element?.props?.replace).toBe(true);
	});
});
