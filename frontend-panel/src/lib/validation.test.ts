import { describe, expect, it, vi } from "vitest";

vi.mock("i18next", () => ({
	default: {
		t: (key: string) => `validation:${key}`,
	},
}));

describe("validation schemas", () => {
	it("accepts valid values", async () => {
		const {
			emailSchema,
			existingPasswordSchema,
			passwordSchema,
			usernameSchema,
		} = await import("@/lib/validation");

		expect(usernameSchema.safeParse("user_1").success).toBe(true);
		expect(emailSchema.safeParse("user@example.com").success).toBe(true);
		expect(existingPasswordSchema.safeParse("pass123").success).toBe(true);
		expect(passwordSchema.safeParse("secret12").success).toBe(true);
	});

	it("returns translated username and email validation messages", async () => {
		const { emailSchema, usernameSchema } = await import("@/lib/validation");

		const username = usernameSchema.safeParse("a!");
		const email = emailSchema.safeParse("bad-email");

		expect(username.success).toBe(false);
		expect(username.error.issues[0]?.message).toBe(
			"validation:username_length",
		);
		expect(email.success).toBe(false);
		expect(email.error.issues[0]?.message).toBe("validation:email_format");
	});

	it("enforces password length boundaries", async () => {
		const { passwordSchema } = await import("@/lib/validation");

		expect(passwordSchema.safeParse("1234567").success).toBe(false);
		expect(passwordSchema.safeParse("x".repeat(129)).success).toBe(false);
	});
});
