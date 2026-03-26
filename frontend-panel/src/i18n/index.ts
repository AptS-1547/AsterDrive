import i18n, { type ResourceKey } from "i18next";
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

const ALL_NAMESPACES = ["common", "files", "auth", "admin", "search"] as const;
const INITIAL_NAMESPACES = ["common", "files", "auth"] as const;
const DEFERRED_NAMESPACES = ["admin", "search"] as const;

type LocaleNamespace = (typeof ALL_NAMESPACES)[number];

async function loadLocale(
	lang: string,
	namespaces: readonly LocaleNamespace[] = ALL_NAMESPACES,
) {
	const entries = await Promise.all(
		namespaces.map(async (namespace) => {
			const module = await import(`./locales/${lang}/${namespace}.json`);
			return [namespace, module.default] as const;
		}),
	);
	return Object.fromEntries(entries) as Partial<
		Record<LocaleNamespace, ResourceKey>
	>;
}

async function ensureNamespaces(
	lang: string,
	namespaces: readonly LocaleNamespace[],
) {
	const missing = namespaces.filter((namespace) => !i18n.hasResourceBundle(lang, namespace));
	if (missing.length === 0) return;
	const resources = await loadLocale(lang, missing);
	for (const [namespace, data] of Object.entries(resources)) {
		i18n.addResourceBundle(lang, namespace, data);
	}
}

const lang = detectLanguage();
const resources = await loadLocale(lang, INITIAL_NAMESPACES);

i18n.use(initReactI18next).init({
	resources: { [lang]: resources },
	lng: lang,
	fallbackLng: "en",
	defaultNS: "common",
	interpolation: { escapeValue: false },
	showSupportNotice: false,
});

void ensureNamespaces(lang, DEFERRED_NAMESPACES);

// 切换语言时按需加载目标语言包
const _changeLanguage = i18n.changeLanguage.bind(i18n);
i18n.changeLanguage = async (newLang?: string, ...args) => {
	if (newLang) {
		await ensureNamespaces(newLang, INITIAL_NAMESPACES);
		void ensureNamespaces(newLang, DEFERRED_NAMESPACES);
	}
	return _changeLanguage(newLang, ...args);
};

export default i18n;
