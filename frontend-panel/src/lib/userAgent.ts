export type UserAgentBrowserFamily =
	| "chrome"
	| "edge"
	| "firefox"
	| "opera"
	| "safari"
	| "samsung-internet"
	| "unknown";

export type UserAgentDeviceType = "desktop" | "mobile" | "tablet" | "unknown";
export type UserAgentOsFamily =
	| "android"
	| "chromeos"
	| "ios"
	| "ipados"
	| "linux"
	| "macos"
	| "windows"
	| "unknown";

export interface ParsedUserAgent {
	browserFamily: UserAgentBrowserFamily;
	browserName: string | null;
	browserVersion: string | null;
	deviceType: UserAgentDeviceType;
	osFamily: UserAgentOsFamily;
	osName: string | null;
}

export interface UserAgentDeviceLabels {
	desktop: string;
	mobile: string;
	tablet: string;
	unknown: string;
}

const DEFAULT_DEVICE_LABELS: UserAgentDeviceLabels = {
	desktop: "Desktop",
	mobile: "Mobile",
	tablet: "Tablet",
	unknown: "Unknown device",
};

function trimUserAgent(userAgent?: string | null): string {
	return userAgent?.trim() ?? "";
}

function isModernIPadOsUserAgent(userAgent: string): boolean {
	return /Macintosh/i.test(userAgent) && /Mobile\/[\w.]+/i.test(userAgent);
}

function formatVersion(version: string, maxSegments = 2): string {
	const parts = version
		.replaceAll("_", ".")
		.split(".")
		.filter((part) => part.length > 0);

	while (parts.length > 1 && parts.at(-1) === "0") {
		parts.pop();
	}

	return parts.slice(0, maxSegments).join(".");
}

function parseBrowser(userAgent: string): {
	browserFamily: UserAgentBrowserFamily;
	browserName: string | null;
	browserVersion: string | null;
} {
	const matchers: Array<[RegExp, UserAgentBrowserFamily, string]> = [
		[/Edg(?:A|iOS)?\/([\d._]+)/i, "edge", "Edge"],
		[/OPR\/([\d._]+)/i, "opera", "Opera"],
		[/SamsungBrowser\/([\d._]+)/i, "samsung-internet", "Samsung Internet"],
		[/Firefox\/([\d._]+)/i, "firefox", "Firefox"],
		[/FxiOS\/([\d._]+)/i, "firefox", "Firefox"],
		[/CriOS\/([\d._]+)/i, "chrome", "Chrome"],
		[/Chrome\/([\d._]+)/i, "chrome", "Chrome"],
	];

	for (const [pattern, browserFamily, browserName] of matchers) {
		const match = userAgent.match(pattern);
		if (!match) {
			continue;
		}

		return {
			browserFamily,
			browserName,
			browserVersion: formatVersion(match[1] ?? ""),
		};
	}

	if (/Safari/i.test(userAgent) && !/(Chrome|CriOS|Edg|OPR)/i.test(userAgent)) {
		const match =
			userAgent.match(/Version\/([\d._]+)/i) ||
			userAgent.match(/Safari\/([\d._]+)/i);

		return {
			browserFamily: "safari",
			browserName: "Safari",
			browserVersion: match ? formatVersion(match[1] ?? "") : null,
		};
	}

	return {
		browserFamily: "unknown",
		browserName: null,
		browserVersion: null,
	};
}

function parseOperatingSystem(userAgent: string): {
	osFamily: UserAgentOsFamily;
	osName: string | null;
} {
	const iPadMatch =
		userAgent.match(/iPad.*OS ([\d_]+)/i) ||
		userAgent.match(/CPU OS ([\d_]+)/i);
	if (iPadMatch) {
		return {
			osFamily: "ipados",
			osName: `iPadOS ${formatVersion(iPadMatch[1] ?? "", 3)}`,
		};
	}

	if (isModernIPadOsUserAgent(userAgent)) {
		return {
			osFamily: "ipados",
			osName: "iPadOS",
		};
	}

	const iPhoneMatch = userAgent.match(/(?:iPhone|CPU iPhone) OS ([\d_]+)/i);
	if (iPhoneMatch) {
		return {
			osFamily: "ios",
			osName: `iOS ${formatVersion(iPhoneMatch[1] ?? "", 3)}`,
		};
	}

	const androidMatch = userAgent.match(/Android ([\d.]+)/i);
	if (androidMatch) {
		return {
			osFamily: "android",
			osName: `Android ${formatVersion(androidMatch[1] ?? "", 3)}`,
		};
	}

	const macMatch = userAgent.match(/Mac OS X ([\d_]+)/i);
	if (macMatch) {
		return {
			osFamily: "macos",
			osName: `macOS ${formatVersion(macMatch[1] ?? "", 3)}`,
		};
	}

	const chromeOsMatch = userAgent.match(/CrOS [^ ]+ ([\d.]+)/i);
	if (chromeOsMatch) {
		return {
			osFamily: "chromeos",
			osName: `ChromeOS ${formatVersion(chromeOsMatch[1] ?? "", 2)}`,
		};
	}

	if (/Windows/i.test(userAgent)) {
		return {
			osFamily: "windows",
			osName: "Windows",
		};
	}

	if (/Linux|X11/i.test(userAgent)) {
		return {
			osFamily: "linux",
			osName: "Linux",
		};
	}

	return {
		osFamily: "unknown",
		osName: null,
	};
}

function parseDeviceType(userAgent: string): UserAgentDeviceType {
	if (/iPad|Tablet/i.test(userAgent)) {
		return "tablet";
	}

	if (isModernIPadOsUserAgent(userAgent)) {
		return "tablet";
	}

	if (/Android/i.test(userAgent) && !/Mobile/i.test(userAgent)) {
		return "tablet";
	}

	if (/Mobile|iPhone|iPod|Windows Phone/i.test(userAgent)) {
		return "mobile";
	}

	if (/Macintosh|Windows|Linux|X11|CrOS/i.test(userAgent)) {
		return "desktop";
	}

	return "unknown";
}

function getDeviceLabel(
	deviceType: UserAgentDeviceType,
	labels: UserAgentDeviceLabels,
): string {
	switch (deviceType) {
		case "desktop":
			return labels.desktop;
		case "mobile":
			return labels.mobile;
		case "tablet":
			return labels.tablet;
		default:
			return labels.unknown;
	}
}

function getPasskeyOsName(parsed: ParsedUserAgent): string | null {
	switch (parsed.osFamily) {
		case "android":
			return "Android";
		case "chromeos":
			return "ChromeOS";
		case "ios":
			return "iOS";
		case "ipados":
			return "iPadOS";
		case "linux":
			return "Linux";
		case "macos":
			return "macOS";
		case "windows":
			return "Windows";
		default:
			return parsed.osName;
	}
}

export function parseUserAgent(userAgent?: string | null): ParsedUserAgent {
	const normalizedUserAgent = trimUserAgent(userAgent);
	if (!normalizedUserAgent) {
		return {
			browserFamily: "unknown",
			browserName: null,
			browserVersion: null,
			deviceType: "unknown",
			osFamily: "unknown",
			osName: null,
		};
	}

	const { browserFamily, browserName, browserVersion } =
		parseBrowser(normalizedUserAgent);
	const { osFamily, osName } = parseOperatingSystem(normalizedUserAgent);

	return {
		browserFamily,
		browserName,
		browserVersion,
		deviceType: parseDeviceType(normalizedUserAgent),
		osFamily,
		osName,
	};
}

export function formatUserAgentLabel(
	userAgent?: string | null,
	labels?: Partial<UserAgentDeviceLabels>,
): string {
	const normalizedUserAgent = trimUserAgent(userAgent);
	if (!normalizedUserAgent) {
		return labels?.unknown ?? DEFAULT_DEVICE_LABELS.unknown;
	}

	const mergedLabels = {
		...DEFAULT_DEVICE_LABELS,
		...labels,
	};
	const parsed = parseUserAgent(normalizedUserAgent);
	const parts: string[] = [];

	if (parsed.browserName) {
		parts.push(
			parsed.browserVersion
				? `${parsed.browserName} ${parsed.browserVersion}`
				: parsed.browserName,
		);
	}

	if (parsed.osName) {
		parts.push(parsed.osName);
	}

	if (parts.length > 0 && parsed.deviceType !== "unknown") {
		parts.push(getDeviceLabel(parsed.deviceType, mergedLabels));
	}

	return parts.length > 0 ? parts.join(" · ") : normalizedUserAgent;
}

export function formatPasskeyDefaultName(
	userAgent?: string | null,
	fallback = "Passkey",
): string {
	const normalizedUserAgent = trimUserAgent(userAgent);
	if (!normalizedUserAgent) {
		return fallback;
	}

	const parsed = parseUserAgent(normalizedUserAgent);
	const osName = getPasskeyOsName(parsed);
	if (!osName || !parsed.browserName) {
		return fallback;
	}

	return `${osName} - ${parsed.browserName}`;
}
