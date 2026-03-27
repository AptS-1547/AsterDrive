import i18next from "i18next";
import { z } from "zod/v4";

function t(key: string): string {
	return i18next.t(key, { ns: "validation" });
}

export const usernameSchema = z
	.string()
	.min(4, t("username_length"))
	.max(16, t("username_length"))
	.regex(/^[a-zA-Z0-9_-]+$/, t("username_chars"));

export const emailSchema = z
	.string()
	.max(254, t("email_too_long"))
	.regex(/^[^@]+@[^@]+\.[^@]+$/, t("email_format"));

export const passwordSchema = z
	.string()
	.min(6, t("password_min"))
	.max(128, t("password_max"));
