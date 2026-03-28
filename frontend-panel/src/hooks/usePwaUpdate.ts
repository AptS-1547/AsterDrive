import { useRegisterSW } from "virtual:pwa-register/react";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { runtimeFlags } from "@/config/runtime";

function logPwaUpdate(message: string, extra?: unknown) {
	if (!runtimeFlags.isDev) return;
	if (extra === undefined) {
		console.debug(`[pwa-update] ${message}`);
		return;
	}
	console.debug(`[pwa-update] ${message}`, extra);
}

export function usePwaUpdate() {
	const { t } = useTranslation();

	const {
		needRefresh: [needRefresh],
		offlineReady: [offlineReady],
		updateServiceWorker,
	} = useRegisterSW({
		onRegistered(r) {
			logPwaUpdate("registered", {
				scope: r?.scope,
				active: r?.active?.scriptURL,
				waiting: r?.waiting?.scriptURL,
				installing: r?.installing?.scriptURL,
			});
			if (r) {
				setInterval(
					() => {
						logPwaUpdate("manual update check");
						void r.update();
					},
					60 * 60 * 1000,
				);
			}
		},
		onRegisterError(error) {
			logPwaUpdate("register error", error);
		},
	});

	useEffect(() => {
		logPwaUpdate("needRefresh changed", needRefresh);
	}, [needRefresh]);

	useEffect(() => {
		logPwaUpdate("offlineReady changed", offlineReady);
	}, [offlineReady]);

	useEffect(() => {
		if (needRefresh) {
			toast.info(t("offline:pwa_update_available"), {
				action: {
					label: t("offline:pwa_refresh"),
					onClick: () => {
						logPwaUpdate("apply update clicked");
						void updateServiceWorker(true);
					},
				},
				duration: Number.POSITIVE_INFINITY,
			});
		}
	}, [needRefresh, t, updateServiceWorker]);
}
