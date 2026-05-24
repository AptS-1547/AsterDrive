import type { TotpSetupStartResponse } from "@/services/authService";

export type PendingAction = "disable" | "regenerate" | null;
export type SetupStep = "intro" | "scan" | "verify" | "recovery";

export const SETUP_STEPS: SetupStep[] = ["intro", "scan", "verify", "recovery"];

export interface SetupUiState {
	setup: TotpSetupStartResponse | null;
	step: SetupStep | null;
	busy: boolean;
	finishBusy: boolean;
	code: string;
	name: string;
	showSecret: boolean;
	recoveryCodes: string[];
	recoveryConfirmed: boolean;
}

export interface ActionUiState {
	kind: PendingAction;
	code: string;
	busy: boolean;
}

export type SetupAction =
	| { type: "reset" }
	| { type: "intro" }
	| { type: "start_busy" }
	| { type: "start_success"; setup: TotpSetupStartResponse }
	| { type: "start_error" }
	| { type: "set_step"; step: SetupStep }
	| { type: "set_code"; code: string }
	| { type: "set_name"; name: string }
	| { type: "toggle_secret" }
	| { type: "finish_busy" }
	| { type: "finish_success"; recoveryCodes: string[] }
	| { type: "finish_error" }
	| { type: "toggle_recovery_confirmed" };

export type ActionFormAction =
	| { type: "open"; kind: Exclude<PendingAction, null> }
	| { type: "reset" }
	| { type: "busy"; busy: boolean }
	| { type: "code"; code: string };

const EMPTY_SETUP_STATE: SetupUiState = {
	setup: null,
	step: null,
	busy: false,
	finishBusy: false,
	code: "",
	name: "",
	showSecret: false,
	recoveryCodes: [],
	recoveryConfirmed: false,
};

export const EMPTY_ACTION_STATE: ActionUiState = {
	kind: null,
	code: "",
	busy: false,
};

export function createSetupState(
	overrides: Partial<SetupUiState> = {},
): SetupUiState {
	return {
		...EMPTY_SETUP_STATE,
		...overrides,
		recoveryCodes: overrides.recoveryCodes ?? [],
	};
}

function normalizeTotpCode(value: string) {
	return value.replace(/\D/g, "").slice(0, 6);
}

export function setupReducer(
	state: SetupUiState,
	action: SetupAction,
): SetupUiState {
	switch (action.type) {
		case "reset":
			return createSetupState();
		case "intro":
			return createSetupState({ step: "intro" });
		case "start_busy":
			return {
				...createSetupState({ step: state.step }),
				busy: true,
			};
		case "start_success":
			return createSetupState({ setup: action.setup, step: "scan" });
		case "start_error":
			return { ...state, busy: false };
		case "set_step":
			return { ...state, step: action.step };
		case "set_code":
			return { ...state, code: normalizeTotpCode(action.code) };
		case "set_name":
			return { ...state, name: action.name };
		case "toggle_secret":
			return { ...state, showSecret: !state.showSecret };
		case "finish_busy":
			return { ...state, finishBusy: true };
		case "finish_success":
			return createSetupState({
				step: "recovery",
				recoveryCodes: action.recoveryCodes,
			});
		case "finish_error":
			return { ...state, finishBusy: false };
		case "toggle_recovery_confirmed":
			return {
				...state,
				recoveryConfirmed: !state.recoveryConfirmed,
			};
	}
}

export function actionReducer(
	state: ActionUiState,
	action: ActionFormAction,
): ActionUiState {
	switch (action.type) {
		case "open":
			return {
				...EMPTY_ACTION_STATE,
				kind: action.kind,
			};
		case "reset":
			return EMPTY_ACTION_STATE;
		case "busy":
			return { ...state, busy: action.busy };
		case "code":
			return { ...state, code: action.code };
	}
}

export function stepIndex(step: SetupStep) {
	return SETUP_STEPS.indexOf(step);
}

export function formatRecoveryCodesFile(codes: string[]) {
	const generatedAt = new Date().toISOString();
	return [
		"AsterDrive MFA recovery codes",
		`Generated at: ${generatedAt}`,
		"",
		"Each code can be used once. Store this file in a password manager or another secure location.",
		"",
		...codes,
		"",
	].join("\n");
}

export function downloadRecoveryCodes(content: string) {
	const blob = new Blob([content], { type: "text/plain;charset=utf-8" });
	const url = URL.createObjectURL(blob);
	const link = document.createElement("a");
	link.href = url;
	link.download = "asterdrive-mfa-recovery-codes.txt";
	document.body.append(link);
	link.click();
	link.remove();
	URL.revokeObjectURL(url);
}
