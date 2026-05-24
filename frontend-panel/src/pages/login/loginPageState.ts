import type { MfaMethod } from "@/services/authService";
import type { PendingActivationState } from "./PendingActivationPanel";

export interface MfaChallengeState {
	expiresAt: number;
	flowToken: string;
	methods: MfaMethod[];
	returnPath: string;
	successMessage: string;
}

export interface MfaPanelState {
	challenge: MfaChallengeState;
	code: string;
	error: string;
	kind: "mfa";
	now: number;
	submitting: boolean;
}

export interface ExternalAuthRecoveryState {
	email: string;
	emailError: string;
	emailSubmitting: boolean;
	flowToken: string;
	mode: "password" | "email";
	password: string;
	passwordError: string;
	passwordIdentifier: string;
	passwordIdentifierError: string;
	passwordSubmitting: boolean;
	returnPath: string;
	sent: boolean;
}

export interface PasswordResetPanelState {
	email: string;
	error: string;
	requesting: boolean;
}

export type AuthPanelState =
	| { kind: "auth" }
	| { kind: "external-auth-recovery"; recovery: ExternalAuthRecoveryState }
	| MfaPanelState
	| {
			kind: "password-reset";
			passwordReset: PasswordResetPanelState;
	  }
	| {
			kind: "pending-activation";
			pendingActivation: PendingActivationState;
	  };

export type AuthPanelAction =
	| { type: "close_external_auth_recovery" }
	| { type: "close_mfa" }
	| { type: "close_password_reset" }
	| { type: "external_email_sent" }
	| { type: "open_auth" }
	| { type: "open_external_auth_recovery"; recovery: ExternalAuthRecoveryState }
	| { type: "open_mfa"; challenge: MfaChallengeState }
	| { type: "open_password_reset"; email: string }
	| { type: "set_external_email"; email: string; error: string }
	| { type: "set_external_email_error"; error: string }
	| { type: "set_external_email_submitting"; submitting: boolean }
	| { type: "set_external_mode"; mode: "password" | "email" }
	| { type: "set_external_password"; password: string; error?: string }
	| {
			type: "set_external_password_errors";
			identifier: string;
			password: string;
	  }
	| { type: "set_external_password_identifier"; identifier: string }
	| { type: "set_external_password_submitting"; submitting: boolean }
	| { type: "set_mfa_code"; code: string }
	| { type: "set_mfa_error"; error: string }
	| { type: "set_mfa_now"; now: number }
	| { type: "set_mfa_submitting"; submitting: boolean }
	| { type: "set_password_reset_email"; email: string; error: string }
	| { type: "set_password_reset_error"; error: string }
	| { type: "set_password_reset_requesting"; requesting: boolean }
	| {
			type: "set_pending_activation";
			pendingActivation: PendingActivationState;
	  };

export const initialAuthPanelState: AuthPanelState = { kind: "auth" };

export function authPanelReducer(
	state: AuthPanelState,
	action: AuthPanelAction,
): AuthPanelState {
	switch (action.type) {
		case "close_external_auth_recovery":
		case "open_auth":
			return initialAuthPanelState;
		case "close_mfa":
			return initialAuthPanelState;
		case "close_password_reset":
			if (state.kind !== "password-reset") return state;
			return initialAuthPanelState;
		case "external_email_sent":
			if (state.kind !== "external-auth-recovery") return state;
			return {
				...state,
				recovery: {
					...state.recovery,
					emailError: "",
					emailSubmitting: false,
					sent: true,
				},
			};
		case "open_external_auth_recovery":
			return {
				kind: "external-auth-recovery",
				recovery: action.recovery,
			};
		case "open_mfa":
			return {
				challenge: action.challenge,
				code: "",
				error: "",
				kind: "mfa",
				now: Date.now(),
				submitting: false,
			};
		case "open_password_reset":
			return {
				kind: "password-reset",
				passwordReset: {
					email: action.email,
					error: "",
					requesting: false,
				},
			};
		case "set_external_email":
			if (state.kind !== "external-auth-recovery") return state;
			return {
				...state,
				recovery: {
					...state.recovery,
					email: action.email,
					emailError: action.error,
				},
			};
		case "set_external_email_error":
			if (state.kind !== "external-auth-recovery") return state;
			return {
				...state,
				recovery: { ...state.recovery, emailError: action.error },
			};
		case "set_external_email_submitting":
			if (state.kind !== "external-auth-recovery") return state;
			return {
				...state,
				recovery: {
					...state.recovery,
					emailSubmitting: action.submitting,
				},
			};
		case "set_external_mode":
			if (state.kind !== "external-auth-recovery") return state;
			return {
				...state,
				recovery: { ...state.recovery, mode: action.mode },
			};
		case "set_external_password":
			if (state.kind !== "external-auth-recovery") return state;
			return {
				...state,
				recovery: {
					...state.recovery,
					password: action.password,
					passwordError: action.error ?? state.recovery.passwordError,
				},
			};
		case "set_external_password_errors":
			if (state.kind !== "external-auth-recovery") return state;
			return {
				...state,
				recovery: {
					...state.recovery,
					passwordError: action.password,
					passwordIdentifierError: action.identifier,
				},
			};
		case "set_external_password_identifier":
			if (state.kind !== "external-auth-recovery") return state;
			return {
				...state,
				recovery: {
					...state.recovery,
					passwordIdentifier: action.identifier,
					passwordIdentifierError:
						action.identifier.trim().length > 0
							? ""
							: state.recovery.passwordIdentifierError,
				},
			};
		case "set_external_password_submitting":
			if (state.kind !== "external-auth-recovery") return state;
			return {
				...state,
				recovery: {
					...state.recovery,
					passwordSubmitting: action.submitting,
				},
			};
		case "set_mfa_code":
			if (state.kind !== "mfa") return state;
			return { ...state, code: action.code, error: "" };
		case "set_mfa_error":
			if (state.kind !== "mfa") return state;
			return { ...state, error: action.error };
		case "set_mfa_now":
			if (state.kind !== "mfa") return state;
			return { ...state, now: action.now };
		case "set_mfa_submitting":
			if (state.kind !== "mfa") return state;
			return { ...state, submitting: action.submitting };
		case "set_password_reset_email":
			if (state.kind !== "password-reset") return state;
			return {
				...state,
				passwordReset: {
					...state.passwordReset,
					email: action.email,
					error: action.error,
				},
			};
		case "set_password_reset_error":
			if (state.kind !== "password-reset") return state;
			return {
				...state,
				passwordReset: {
					...state.passwordReset,
					error: action.error,
				},
			};
		case "set_password_reset_requesting":
			if (state.kind !== "password-reset") return state;
			return {
				...state,
				passwordReset: {
					...state.passwordReset,
					requesting: action.requesting,
				},
			};
		case "set_pending_activation":
			return {
				kind: "pending-activation",
				pendingActivation: action.pendingActivation,
			};
	}
}
