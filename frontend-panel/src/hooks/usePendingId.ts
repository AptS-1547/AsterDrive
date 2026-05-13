import { useCallback, useRef, useState } from "react";

export function usePendingId<T>() {
	const [pendingId, setPendingId] = useState<T | null>(null);
	const pendingIdRef = useRef<T | null>(null);

	const runWithPending = useCallback(
		async (id: T, action: () => Promise<void>) => {
			if (pendingIdRef.current !== null) {
				return false;
			}

			pendingIdRef.current = id;
			setPendingId(id);
			try {
				await action();
				return true;
			} finally {
				if (Object.is(pendingIdRef.current, id)) {
					pendingIdRef.current = null;
					setPendingId(null);
				}
			}
		},
		[],
	);

	const clearPending = useCallback(() => {
		pendingIdRef.current = null;
		setPendingId(null);
	}, []);

	return {
		clearPending,
		pendingId,
		runWithPending,
	};
}
