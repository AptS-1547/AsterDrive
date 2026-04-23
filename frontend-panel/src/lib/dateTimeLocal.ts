export function toDateTimeLocalValue(value: string | null | undefined): string {
	if (!value) return "";

	const date = new Date(value);
	if (Number.isNaN(date.getTime())) {
		return "";
	}

	const offsetMs = date.getTimezoneOffset() * 60 * 1000;
	return new Date(date.getTime() - offsetMs).toISOString().slice(0, 16);
}

export function toIsoDateTime(value: string): string | null {
	const trimmed = value.trim();
	if (!trimmed) return null;

	const date = new Date(trimmed);
	return Number.isNaN(date.getTime()) ? null : date.toISOString();
}
