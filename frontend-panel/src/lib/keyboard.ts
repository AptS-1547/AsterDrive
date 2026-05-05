export const IME_COMPOSITION_END_GRACE_MS = 32;

interface KeyboardEventLike {
	key: string;
	isComposing?: boolean;
	keyCode?: number;
	nativeEvent?: {
		isComposing?: boolean;
		keyCode?: number;
	};
}

interface ImeComposingKeyEventOptions {
	lastCompositionEndAt?: number;
	now?: number;
}

export function shouldIgnoreKeyboardTarget(target: EventTarget | null) {
	if (!(target instanceof HTMLElement)) return false;

	return (
		target.tagName === "INPUT" ||
		target.tagName === "TEXTAREA" ||
		target.tagName === "SELECT" ||
		target.isContentEditable
	);
}

export function isImeComposingKeyEvent(
	event: KeyboardEventLike,
	options: ImeComposingKeyEventOptions = {},
) {
	const isComposing =
		event.nativeEvent?.isComposing ?? event.isComposing ?? false;
	const keyCode = event.nativeEvent?.keyCode ?? event.keyCode;

	if (isComposing || keyCode === 229 || event.key === "Process") {
		return true;
	}

	const lastCompositionEndAt = options.lastCompositionEndAt ?? 0;
	if (lastCompositionEndAt <= 0) {
		return false;
	}

	const now = options.now ?? Date.now();
	const elapsed = now - lastCompositionEndAt;

	return elapsed >= 0 && elapsed < IME_COMPOSITION_END_GRACE_MS;
}
