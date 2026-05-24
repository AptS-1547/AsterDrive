import type { FormEvent } from "react";
import type { MeResponse } from "@/types/api";
import { SecurityEmailSection } from "./SecurityEmailSection";
import { SecurityPasswordSection } from "./SecurityPasswordSection";
import type { SecurityFormErrors } from "./types";

interface SecurityAccountPaneProps {
	canSubmitEmailChange: boolean;
	canSubmitPassword: boolean;
	confirmPassword: string;
	currentPassword: string;
	emailBusy: boolean;
	emailError?: string;
	errors: SecurityFormErrors;
	newEmail: string;
	newPassword: string;
	passwordBusy: boolean;
	resendingEmailChange: boolean;
	user: MeResponse | null;
	onConfirmPasswordChange: (value: string) => void;
	onCurrentPasswordChange: (value: string) => void;
	onEmailSubmit: (event: FormEvent<HTMLFormElement>) => void;
	onNewEmailChange: (value: string) => void;
	onNewPasswordChange: (value: string) => void;
	onPasswordSubmit: (event: FormEvent<HTMLFormElement>) => void;
	onResendEmailChange: () => void;
}

export function SecurityAccountPane({
	canSubmitEmailChange,
	canSubmitPassword,
	confirmPassword,
	currentPassword,
	emailBusy,
	emailError,
	errors,
	newEmail,
	newPassword,
	onConfirmPasswordChange,
	onCurrentPasswordChange,
	onEmailSubmit,
	onNewEmailChange,
	onNewPasswordChange,
	onPasswordSubmit,
	onResendEmailChange,
	passwordBusy,
	resendingEmailChange,
	user,
}: SecurityAccountPaneProps) {
	return (
		<>
			<SecurityEmailSection
				canSubmitEmailChange={canSubmitEmailChange}
				emailBusy={emailBusy}
				emailError={emailError}
				newEmail={newEmail}
				resendingEmailChange={resendingEmailChange}
				user={user}
				onNewEmailChange={onNewEmailChange}
				onResendEmailChange={onResendEmailChange}
				onSubmit={onEmailSubmit}
			/>

			<SecurityPasswordSection
				canSubmitPassword={canSubmitPassword}
				confirmPassword={confirmPassword}
				currentPassword={currentPassword}
				errors={errors}
				newPassword={newPassword}
				passwordBusy={passwordBusy}
				onConfirmPasswordChange={onConfirmPasswordChange}
				onCurrentPasswordChange={onCurrentPasswordChange}
				onNewPasswordChange={onNewPasswordChange}
				onSubmit={onPasswordSubmit}
			/>
		</>
	);
}
