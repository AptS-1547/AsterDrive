import { useCallback, useEffect, useState } from "react";
import { handleApiError } from "@/hooks/useApiError";

export function useApiList<T>(
	fetcher: () => Promise<{ items: T[]; total?: number }>,
	deps: unknown[] = [],
) {
	const [items, setItems] = useState<T[]>([]);
	const [total, setTotal] = useState(0);
	const [loading, setLoading] = useState(true);

	const load = useCallback(async () => {
		try {
			setLoading(true);
			const data = await fetcher();
			setItems(data.items);
			if (data.total !== undefined) setTotal(data.total);
		} catch (e) {
			handleApiError(e);
		} finally {
			setLoading(false);
		}
		// biome-ignore lint/correctness/useExhaustiveDependencies: deps is a dynamic parameter by design
	}, deps);

	useEffect(() => {
		void load().catch(() => {});
	}, [load]);

	return { items, setItems, total, loading, reload: load };
}
