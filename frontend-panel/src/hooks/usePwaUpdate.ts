import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { useRegisterSW } from "virtual:pwa-register/react";
import { toast } from "sonner";

export function usePwaUpdate() {
	const { t } = useTranslation();

	const {
		needRefresh: [needRefresh],
		updateServiceWorker,
	} = useRegisterSW({
		onRegistered(r) {
			// 每小时检查一次更新
			r && setInterval(() => r.update(), 60 * 60 * 1000);
		},
	});

	useEffect(() => {
		if (needRefresh) {
			toast.info(t("pwa_update_available"), {
				action: {
					label: t("pwa_refresh"),
					onClick: () => updateServiceWorker(true),
				},
				duration: Number.POSITIVE_INFINITY,
			});
		}
	}, [needRefresh, t, updateServiceWorker]);
}
