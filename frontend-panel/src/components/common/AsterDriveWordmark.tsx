import type { ComponentProps } from "react";
import { cn } from "@/lib/utils";
import { useThemeStore } from "@/stores/themeStore";

const WORDMARK_DARK_SRC = "/static/asterdrive/asterdrive-dark.svg";
const WORDMARK_LIGHT_SRC = "/static/asterdrive/asterdrive-light.svg";

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
	const resolvedTheme = useThemeStore((s) => s.resolvedTheme);
	const effectiveTheme =
		surfaceTheme ??
		(resolvedTheme === "dark" || resolveDocumentTheme() === "dark"
			? "dark"
			: "light");

	return (
		<img
			src={effectiveTheme === "dark" ? WORDMARK_LIGHT_SRC : WORDMARK_DARK_SRC}
			alt={alt}
			draggable={draggable}
			className={cn("block h-auto w-auto", className)}
			{...props}
		/>
	);
}
