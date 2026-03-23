import { useEffect, useState } from "react";
import { api } from "@/services/http";

/** 通过 axios 加载文件 blob，走拦截器支持 token refresh */
export function useBlobUrl(path: string | null) {
	const [blobUrl, setBlobUrl] = useState<string | null>(null);
	const [error, setError] = useState(false);
	const [loading, setLoading] = useState(false);

	useEffect(() => {
		setBlobUrl(null);
		setError(false);
		if (!path) {
			setLoading(false);
			return;
		}
		let revoke: string | null = null;
		setLoading(true);
		api.client
			.get(path, { responseType: "blob" })
			.then((r) => {
				const objectUrl = URL.createObjectURL(r.data);
				revoke = objectUrl;
				setBlobUrl(objectUrl);
			})
			.catch(() => setError(true))
			.finally(() => setLoading(false));
		return () => {
			if (revoke) URL.revokeObjectURL(revoke);
		};
	}, [path]);

	return { blobUrl, error, loading };
}
