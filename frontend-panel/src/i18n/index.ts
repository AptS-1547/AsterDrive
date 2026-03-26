import i18n from "i18next";
import { initReactI18next } from "react-i18next";

function detectLanguage(): "en" | "zh" {
	try {
		const stored = localStorage.getItem("aster-language");
		if (stored === "en" || stored === "zh") return stored;
	} catch {
		// ignore
	}
	return navigator.language?.startsWith("zh") ? "zh" : "en";
}

async function loadLocale(lang: string) {
	const [common, files, auth, admin, search] = await Promise.all([
		import(`./locales/${lang}/common.json`),
		import(`./locales/${lang}/files.json`),
		import(`./locales/${lang}/auth.json`),
		import(`./locales/${lang}/admin.json`),
		import(`./locales/${lang}/search.json`),
	]);
	return {
		common: common.default,
		files: files.default,
		auth: auth.default,
		admin: admin.default,
		search: search.default,
	};
}

const lang = detectLanguage();
const resources = await loadLocale(lang);

i18n.use(initReactI18next).init({
	resources: { [lang]: resources },
	lng: lang,
	fallbackLng: "en",
	defaultNS: "common",
	interpolation: { escapeValue: false },
	showSupportNotice: false,
});

// 切换语言时按需加载目标语言包
const _changeLanguage = i18n.changeLanguage.bind(i18n);
i18n.changeLanguage = async (newLang?: string, ...args) => {
	if (newLang && !i18n.hasResourceBundle(newLang, "common")) {
		const r = await loadLocale(newLang);
		for (const [ns, data] of Object.entries(r)) {
			i18n.addResourceBundle(newLang, ns, data);
		}
	}
	return _changeLanguage(newLang, ...args);
};

export default i18n;
