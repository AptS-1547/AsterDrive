import { useCallback, useState } from "react";

interface UseRetainedDialogValueResult<T> {
	handleOpenChangeComplete: (open: boolean) => void;
	retainedValue: T | null;
}

export function useRetainedDialogValue<T>(
	value: T | null,
	open: boolean,
): UseRetainedDialogValueResult<T> {
	const [retainedValue, setRetainedValue] = useState<T | null>(value);

	if (value !== null && retainedValue !== value) {
		setRetainedValue(value);
	} else if (value === null && open && retainedValue !== null) {
		setRetainedValue(null);
	}

	const visibleValue = value ?? (open ? null : retainedValue);

	const handleOpenChangeComplete = useCallback((nextOpen: boolean) => {
		if (!nextOpen) {
			setRetainedValue(null);
		}
	}, []);

	return { retainedValue: visibleValue, handleOpenChangeComplete };
}
