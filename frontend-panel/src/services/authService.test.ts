import { describe, expect, it, vi } from "vitest";
import { authService } from "@/services/authService";

const mockState = vi.hoisted(() => ({
	clientPost: vi.fn(),
	delete: vi.fn(),
	get: vi.fn(),
	patch: vi.fn(),
	post: vi.fn(),
	put: vi.fn(),
}));

vi.mock("@/services/http", () => ({
	api: {
		client: {
			post: mockState.clientPost,
		},
		delete: mockState.delete,
		get: mockState.get,
		patch: mockState.patch,
		post: mockState.post,
		put: mockState.put,
	},
	ApiError: class ApiError extends Error {
		code: number;
		constructor(code: number, message: string) {
			super(message);
			this.code = code;
		}
	},
}));

describe("authService", () => {
	it("uses the expected auth endpoints and payloads", async () => {
		const prefs = {
			language: "zh",
			sort_by: "updated_at",
		};
		mockState.post.mockImplementation((url: string) => {
			if (url === "/auth/login" || url === "/auth/refresh") {
				return { expires_in: 900 };
			}
			return undefined;
		});
		mockState.put.mockImplementation((url: string) => {
			if (url === "/auth/password") {
				return { expires_in: 900 };
			}
			return undefined;
		});
		mockState.get.mockImplementation((url: string) => {
			if (url === "/auth/sessions") {
				return [];
			}
			return undefined;
		});
		mockState.delete.mockImplementation((url: string) => {
			if (url === "/auth/sessions/others") {
				return { removed: 2 };
			}
			return undefined;
		});

		authService.check();
		await expect(
			authService.login("alice@example.com", "secret"),
		).resolves.toEqual({
			expiresIn: 900,
		});
		authService.register("alice", "alice@example.com", "secret");
		authService.resendRegisterActivation("alice@example.com");
		authService.setup("owner", "owner@example.com", "secret");
		authService.logout();
		await expect(authService.refreshToken()).resolves.toEqual({
			expiresIn: 900,
		});
		authService.me();
		authService.me(["quota"]);
		authService.updatePreferences(prefs);
		await expect(
			authService.changePassword({
				current_password: "secret",
				new_password: "newsecret",
			}),
		).resolves.toEqual({
			expiresIn: 900,
		});
		authService.updateProfile({ display_name: "Alice" });
		authService.requestEmailChange("alice+next@example.com");
		authService.resendEmailChange();
		authService.setAvatarSource("gravatar");
		expect(authService.listSessions()).toEqual([]);
		authService.revokeSession("session-1");
		await expect(authService.revokeOtherSessions()).resolves.toBe(2);

		expect(mockState.post).toHaveBeenNthCalledWith(1, "/auth/check");
		expect(mockState.post).toHaveBeenNthCalledWith(2, "/auth/login", {
			identifier: "alice@example.com",
			password: "secret",
		});
		expect(mockState.post).toHaveBeenNthCalledWith(3, "/auth/register", {
			username: "alice",
			email: "alice@example.com",
			password: "secret",
		});
		expect(mockState.post).toHaveBeenNthCalledWith(4, "/auth/register/resend", {
			identifier: "alice@example.com",
		});
		expect(mockState.post).toHaveBeenNthCalledWith(5, "/auth/setup", {
			username: "owner",
			email: "owner@example.com",
			password: "secret",
		});
		expect(mockState.post).toHaveBeenNthCalledWith(6, "/auth/logout");
		expect(mockState.post).toHaveBeenNthCalledWith(7, "/auth/refresh");
		expect(mockState.get).toHaveBeenNthCalledWith(1, "/auth/me");
		expect(mockState.get).toHaveBeenNthCalledWith(2, "/auth/me", {
			params: { fields: "quota" },
		});
		expect(mockState.patch).toHaveBeenNthCalledWith(
			1,
			"/auth/preferences",
			prefs,
		);
		expect(mockState.put).toHaveBeenNthCalledWith(1, "/auth/password", {
			current_password: "secret",
			new_password: "newsecret",
		});
		expect(mockState.patch).toHaveBeenNthCalledWith(2, "/auth/profile", {
			display_name: "Alice",
		});
		expect(mockState.post).toHaveBeenNthCalledWith(8, "/auth/email/change", {
			new_email: "alice+next@example.com",
		});
		expect(mockState.post).toHaveBeenNthCalledWith(
			9,
			"/auth/email/change/resend",
		);
		expect(mockState.put).toHaveBeenNthCalledWith(
			2,
			"/auth/profile/avatar/source",
			{
				source: "gravatar",
			},
		);
		expect(mockState.get).toHaveBeenNthCalledWith(3, "/auth/sessions");
		expect(mockState.delete).toHaveBeenNthCalledWith(
			1,
			"/auth/sessions/session-1",
		);
		expect(mockState.delete).toHaveBeenNthCalledWith(
			2,
			"/auth/sessions/others",
		);
	});
});
