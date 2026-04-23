export { toDateTimeLocalValue, toIsoDateTime } from "@/lib/dateTimeLocal";

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

export function normalizeMaxDownloads(value: string): number {
	const parsed = Number.parseInt(value, 10);
	if (Number.isNaN(parsed) || parsed < 0) {
		return 0;
	}

	return parsed;
}
