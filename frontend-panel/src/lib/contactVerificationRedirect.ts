export type ContactVerificationRedirectStatus =
	| "email-changed"
	| "expired"
	| "invalid"
	| "missing"
	| "register-activated";

const STATUSES = new Set<ContactVerificationRedirectStatus>([
	"email-changed",
	"expired",
	"invalid",
	"missing",
	"register-activated",
]);

export interface ContactVerificationRedirectState {
	email: string | null;
	status: ContactVerificationRedirectStatus;
}

export function getContactVerificationRedirectState(
	search: string,
): ContactVerificationRedirectState | null {
	const params = new URLSearchParams(search);
	const status = params.get("contact_verification")?.trim();
	if (!status || !STATUSES.has(status as ContactVerificationRedirectStatus)) {
		return null;
	}

	const email = params.get("email")?.trim() || null;
	return {
		email,
		status: status as ContactVerificationRedirectStatus,
	};
}

export function clearContactVerificationRedirectSearch(search: string) {
	const params = new URLSearchParams(search);
	params.delete("contact_verification");
	params.delete("email");
	const next = params.toString();
	return next ? `?${next}` : "";
}
