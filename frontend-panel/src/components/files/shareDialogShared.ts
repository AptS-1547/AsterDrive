export function computeShareExpiry(value: string): string | null {
	if (value === "never") return null;

	const now = new Date();
	switch (value) {
		case "1h":
			now.setHours(now.getHours() + 1);
			break;
		case "1d":
			now.setDate(now.getDate() + 1);
			break;
		case "7d":
			now.setDate(now.getDate() + 7);
			break;
		case "30d":
			now.setDate(now.getDate() + 30);
			break;
		default:
			return null;
	}

	return now.toISOString();
}

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

export function normalizeMaxDownloads(value: string): number {
	const parsed = Number.parseInt(value, 10);
	if (Number.isNaN(parsed) || parsed < 0) {
		return 0;
	}

	return parsed;
}
