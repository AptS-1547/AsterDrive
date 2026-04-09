import type { ComponentProps } from "react";
import { cn } from "@/lib/utils";
import { useBrandingStore } from "@/stores/brandingStore";
import { useThemeStore } from "@/stores/themeStore";

type SurfaceTheme = "light" | "dark";

function resolveDocumentTheme() {
	if (typeof document === "undefined") return "light";
	return document.documentElement.classList.contains("dark") ? "dark" : "light";
}

type AsterDriveWordmarkProps = Omit<ComponentProps<"img">, "src"> & {
	surfaceTheme?: SurfaceTheme;
};

export function AsterDriveWordmark({
	alt,
	className,
	draggable = false,
	surfaceTheme,
	...props
}: AsterDriveWordmarkProps) {
	const wordmarkDarkUrl = useBrandingStore((s) => s.branding.wordmarkDarkUrl);
	const wordmarkLightUrl = useBrandingStore((s) => s.branding.wordmarkLightUrl);
	const resolvedTheme = useThemeStore((s) => s.resolvedTheme);
	const effectiveTheme =
		surfaceTheme ??
		(resolvedTheme === "dark" || resolveDocumentTheme() === "dark"
			? "dark"
			: "light");

	return (
		<img
			src={effectiveTheme === "dark" ? wordmarkLightUrl : wordmarkDarkUrl}
			alt={alt}
			draggable={draggable}
			className={cn("block h-auto w-auto", className)}
			{...props}
		/>
	);
}
