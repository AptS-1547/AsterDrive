import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ExternalAuthRecoveryPanel } from "@/pages/login/ExternalAuthRecoveryPanel";

const validEmailSchema = {
	safeParse: () => ({ success: true }),
};

function renderPanel(
	overrides: Partial<
		React.ComponentProps<typeof ExternalAuthRecoveryPanel>
	> = {},
) {
	return render(
		<ExternalAuthRecoveryPanel
			email="verify@example.com"
			emailError=""
			emailSchema={validEmailSchema as never}
			identifier="user@example.com"
			identifierError=""
			mode="password"
			password="secret123"
			passwordError=""
			sent={false}
			submittingEmail={false}
			submittingPassword={false}
			t={(key) => key.replace(/^core:/, "")}
			onBack={vi.fn()}
			onEmailChange={vi.fn()}
			onIdentifierChange={vi.fn()}
			onModeChange={vi.fn()}
			onPasswordChange={vi.fn()}
			{...overrides}
		/>,
	);
}

describe("ExternalAuthRecoveryPanel", () => {
	it("uses a submit button for password linking so Enter submits the parent form", () => {
		renderPanel({ mode: "password" });

		expect(
			screen.getByRole("button", {
				name: /external_auth_password_link_submit/,
			}),
		).toHaveAttribute("type", "submit");
	});

	it("uses a submit button for email verification so Enter submits the parent form", () => {
		renderPanel({ mode: "email" });

		expect(
			screen.getByRole("button", {
				name: /external_auth_email_verification_send/,
			}),
		).toHaveAttribute("type", "submit");
	});
});
