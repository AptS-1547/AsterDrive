import type { OpenWithOption } from "./types";

function getEmbeddedOptionMode(option: OpenWithOption | null) {
	if (!option) {
		return "new_tab";
	}

	if (option.mode !== "url_template" && option.mode !== "wopi") {
		return "iframe";
	}

	return option.config?.mode === "new_tab" ? "new_tab" : "iframe";
}

export function optionUsesInnerScroll(option: OpenWithOption | null) {
	return (
		option?.mode === "pdf" ||
		option?.mode === "table" ||
		((option?.mode === "url_template" || option?.mode === "wopi") &&
			getEmbeddedOptionMode(option) !== "new_tab")
	);
}

export function optionFillsViewportHeight(option: OpenWithOption | null) {
	return (
		option?.mode === "code" ||
		option?.mode === "formatted" ||
		option?.mode === "markdown" ||
		option?.mode === "archive" ||
		option?.mode === "pdf" ||
		option?.mode === "table" ||
		((option?.mode === "url_template" || option?.mode === "wopi") &&
			getEmbeddedOptionMode(option) !== "new_tab")
	);
}

export function getDialogContentClassName({
	fillsViewportHeight,
	isExpanded,
	showOpenMethodChooser,
}: {
	fillsViewportHeight: boolean;
	isExpanded: boolean;
	showOpenMethodChooser: boolean;
}) {
	if (showOpenMethodChooser) {
		return "flex max-h-[min(90vh,calc(100vh-2rem))] w-[min(96vw,32rem)] max-w-[min(96vw,32rem)] flex-col gap-0 overflow-hidden p-0 sm:max-w-[min(96vw,32rem)]";
	}

	return [
		"flex max-h-[90vh] w-[min(96vw,1200px)] max-w-[min(96vw,1200px)] flex-col gap-0 overflow-hidden p-0 sm:max-w-[min(96vw,1200px)]",
		(fillsViewportHeight || isExpanded) && "h-[90vh]",
		isExpanded &&
			"top-0 left-0 h-screen w-screen max-h-screen max-w-none translate-x-0 translate-y-0 rounded-none sm:max-w-none",
	]
		.filter(Boolean)
		.join(" ");
}
