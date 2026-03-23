import { useCallback, useEffect, useState } from "react";
import { api } from "@/services/http";

export function useTextContent(path: string | null) {
	const [content, setContent] = useState<string | null>(null);
	const [etag, setEtag] = useState<string | null>(null);
	const [loading, setLoading] = useState(false);
	const [error, setError] = useState(false);

	const load = useCallback(async () => {
		if (!path) {
			setContent(null);
			setEtag(null);
			setLoading(false);
			setError(false);
			return;
		}

		setLoading(true);
		setError(false);
		try {
			const response = await api.client.get(path, { responseType: "text" });
			setContent(response.data);
			setEtag(response.headers.etag ?? null);
		} catch {
			setError(true);
		} finally {
			setLoading(false);
		}
	}, [path]);

	useEffect(() => {
		load();
	}, [load]);

	return {
		content,
		etag,
		loading,
		error,
		reload: load,
		setContent,
		setEtag,
	};
}
