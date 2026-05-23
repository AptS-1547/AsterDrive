import type { ComponentType } from "react";
import {
	FaAndroid,
	FaApple,
	FaChrome,
	FaDisplay,
	FaEdge,
	FaFirefoxBrowser,
	FaLinux,
	FaMobileScreenButton,
	FaOpera,
	FaSafari,
	FaTabletScreenButton,
	FaWindows,
} from "react-icons/fa6";
import { PiBrowsers, PiDesktop, PiGlobeHemisphereWest } from "react-icons/pi";
import {
	parseUserAgent,
	type UserAgentBrowserFamily,
	type UserAgentDeviceType,
	type UserAgentOsFamily,
} from "@/lib/userAgent";
import { cn } from "@/lib/utils";

type SessionIconComponent = ComponentType<{ className?: string }>;

interface SessionIconPresentation {
	className: string;
	Icon: SessionIconComponent;
}

const GENERIC_BROWSER_PRESENTATION: SessionIconPresentation = {
	className: "text-primary",
	Icon: PiGlobeHemisphereWest,
};

const APPLE_PLATFORM_PRESENTATION: SessionIconPresentation = {
	className: "text-slate-700 dark:text-slate-200",
	Icon: FaApple,
};

const BROWSER_PRESENTATIONS: Partial<
	Record<UserAgentBrowserFamily, SessionIconPresentation>
> = {
	chrome: {
		className: "text-amber-500 dark:text-amber-400",
		Icon: FaChrome,
	},
	edge: {
		className: "text-sky-600 dark:text-sky-400",
		Icon: FaEdge,
	},
	firefox: {
		className: "text-orange-500 dark:text-orange-400",
		Icon: FaFirefoxBrowser,
	},
	opera: {
		className: "text-red-600 dark:text-red-400",
		Icon: FaOpera,
	},
	safari: {
		className: "text-blue-500 dark:text-blue-400",
		Icon: FaSafari,
	},
	"samsung-internet": {
		className: "text-indigo-600 dark:text-indigo-400",
		Icon: PiBrowsers,
	},
};

const DEVICE_FALLBACK_PRESENTATIONS: Record<
	UserAgentDeviceType,
	SessionIconPresentation
> = {
	desktop: {
		className: "text-slate-600 dark:text-slate-300",
		Icon: PiDesktop,
	},
	mobile: {
		className: "text-cyan-600 dark:text-cyan-400",
		Icon: FaMobileScreenButton,
	},
	tablet: {
		className: "text-violet-600 dark:text-violet-400",
		Icon: FaTabletScreenButton,
	},
	unknown: {
		className: "text-muted-foreground",
		Icon: FaDisplay,
	},
};

const PLATFORM_PRESENTATIONS: Partial<
	Record<UserAgentOsFamily, SessionIconPresentation>
> = {
	android: {
		className: "text-emerald-600 dark:text-emerald-400",
		Icon: FaAndroid,
	},
	chromeos: {
		className: "text-amber-500 dark:text-amber-400",
		Icon: FaChrome,
	},
	ios: APPLE_PLATFORM_PRESENTATION,
	ipados: APPLE_PLATFORM_PRESENTATION,
	linux: {
		className: "text-amber-600 dark:text-amber-400",
		Icon: FaLinux,
	},
	macos: APPLE_PLATFORM_PRESENTATION,
	windows: {
		className: "text-blue-600 dark:text-blue-400",
		Icon: FaWindows,
	},
};

function getBrowserPresentation(
	browserFamily: UserAgentBrowserFamily,
): SessionIconPresentation {
	return BROWSER_PRESENTATIONS[browserFamily] ?? GENERIC_BROWSER_PRESENTATION;
}

function getDeviceFallbackPresentation(
	deviceType: UserAgentDeviceType,
): SessionIconPresentation {
	return DEVICE_FALLBACK_PRESENTATIONS[deviceType];
}

function getPlatformPresentation(
	osFamily: UserAgentOsFamily,
	deviceType: UserAgentDeviceType,
): SessionIconPresentation {
	return (
		PLATFORM_PRESENTATIONS[osFamily] ??
		getDeviceFallbackPresentation(deviceType)
	);
}

interface SessionPlatformIconProps {
	userAgent?: string | null;
	className?: string;
}

export function SessionPlatformIcon({
	userAgent,
	className,
}: SessionPlatformIconProps) {
	const parsed = parseUserAgent(userAgent);
	const browser = getBrowserPresentation(parsed.browserFamily);
	const platform = getPlatformPresentation(parsed.osFamily, parsed.deviceType);

	return (
		<span
			className={cn(
				"relative flex size-8 shrink-0 items-center justify-center",
				className,
			)}
			aria-hidden="true"
		>
			<browser.Icon className={cn("size-4", browser.className)} />
			<span className="absolute -right-1 -bottom-1 flex size-4 items-center justify-center rounded-full border border-background bg-background shadow-sm">
				<platform.Icon className={cn("h-2.5 w-2.5", platform.className)} />
			</span>
		</span>
	);
}
