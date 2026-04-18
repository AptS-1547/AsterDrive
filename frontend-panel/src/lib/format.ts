import type { i18n as I18n } from "i18next";

const INTEGER_FORMATTER = new Intl.NumberFormat();

type DateFormatI18n = Pick<I18n, "language" | "resolvedLanguage" | "t">;

function getDateLocale(i18n: DateFormatI18n): string | undefined {
	return i18n.resolvedLanguage ?? (i18n.language || undefined);
}

export function formatBytes(bytes: number): string {
	if (bytes === 0) return "0 B";
	const k = 1024;
	const sizes = ["B", "KB", "MB", "GB", "TB"];
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	return `${(bytes / k ** i).toFixed(1)} ${sizes[i]}`;
}

export function formatNumber(value: number): string {
	if (!Number.isFinite(value)) {
		return String(value);
	}
	return INTEGER_FORMATTER.format(value);
}

export function formatDate(dateStr: string, i18n: DateFormatI18n): string {
	const date = new Date(dateStr);
	const now = new Date();
	const diff = now.getTime() - date.getTime();
	const minutes = Math.floor(diff / 60000);
	if (minutes < 1) return i18n.t("core:date_relative_just_now");
	if (minutes < 60) {
		return i18n.t("core:date_relative_minutes_ago", { count: minutes });
	}
	const hours = Math.floor(minutes / 60);
	if (hours < 24) {
		return i18n.t("core:date_relative_hours_ago", { count: hours });
	}
	const days = Math.floor(hours / 24);
	if (days < 30) {
		return i18n.t("core:date_relative_days_ago", { count: days });
	}
	return date.toLocaleDateString(getDateLocale(i18n));
}

export function formatDateAbsolute(dateStr: string): string {
	return new Date(dateStr).toLocaleString();
}

export function formatDateShort(dateStr: string): string {
	return new Date(dateStr).toLocaleDateString();
}

export function formatDateTime(dateStr: string): string {
	return new Date(dateStr).toLocaleString();
}
