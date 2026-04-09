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

	it("registers admin mail settings routes without the removed verify-contact page", async () => {
		await import("./index");

		const routes = createBrowserRouterMock.mock.calls[0]?.[0] as Array<{
			children?: Array<{
				children?: Array<unknown>;
				element?: {
					props?: {
						to?: string;
					};
				};
				path?: string;
			}>;
			element?: {
				props?: {
					to?: string;
				};
			};
			path?: string;
		}>;
		const flattenRoutes = (
			items: Array<{
				children?: Array<unknown>;
				element?: {
					props?: {
						to?: string;
					};
				};
				path?: string;
			}>,
		): Array<{
			element?: {
				props?: {
					to?: string;
				};
			};
			path?: string;
		}> =>
			items.flatMap((route) => [
				route,
				...flattenRoutes(
					(route.children as Array<{
						children?: Array<unknown>;
						element?: {
							props?: {
								to?: string;
							};
						};
						path?: string;
					}>) ?? [],
				),
			]);
		const allRoutes = flattenRoutes(routes);

		expect(
			allRoutes.some((route) => route.path === "/admin/settings/user"),
		).toBe(true);
		expect(
			allRoutes.some((route) => route.path === "/admin/settings/mail"),
		).toBe(true);
		expect(allRoutes.some((route) => route.path === "/verify-contact")).toBe(
			false,
		);
		expect(
			allRoutes.find((route) => route.path === "/admin/settings")?.element
				?.props?.to,
		).toBe("/admin/settings/general");
		expect(
			allRoutes.find((route) => route.path === "/admin/settings/:section")
				?.element?.props?.to,
		).toBe("/admin/settings/general");
	});
});
