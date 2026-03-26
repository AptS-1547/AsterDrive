import { useRegisterSW } from "virtual:pwa-register/react";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";

const IS_DEV = import.meta.env.DEV;

function logPwaUpdate(message: string, extra?: unknown) {
	if (!IS_DEV) return;
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
			toast.info(t("pwa_update_available"), {
				action: {
					label: t("pwa_refresh"),
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
