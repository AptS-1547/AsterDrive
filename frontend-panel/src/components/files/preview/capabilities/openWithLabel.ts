import { pickLocalizedLabel } from "@/lib/localizedLabel";
import type { OpenWithOption } from "./types";

export function resolveOpenWithOptionLabel(
	option: OpenWithOption,
	language: string | undefined,
	t: (key: string) => string,
) {
	const dynamicLabel = pickLocalizedLabel(option.labels, language);
	if (dynamicLabel) {
		return dynamicLabel;
	}

	if (option.label?.trim()) {
		return option.label.trim();
	}

	if (option.labelKey) {
		return t(option.labelKey);
	}

	return option.key;
}
