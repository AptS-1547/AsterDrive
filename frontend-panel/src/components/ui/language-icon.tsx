import type { ComponentType } from "react";
import { useEffect, useState } from "react";
import { cn } from "@/lib/utils";

type DevIconComponent = ComponentType<{ size?: string | number }>;

let cachedResolve: ((name: string) => DevIconComponent | null) | null = null;
let cachedCheck: ((name: string) => boolean) | null = null;
let loadPromise: Promise<void> | null = null;

function ensureLoaded(): Promise<void> {
	if (cachedResolve) return Promise.resolve();
	if (!loadPromise) {
		loadPromise = import("./language-icons-map").then((mod) => {
			cachedResolve = mod.resolveIcon;
			cachedCheck = mod.checkHasIcon;
		});
	}
	return loadPromise;
}

/** Synchronous check — only returns true if the chunk is loaded AND a matching icon exists. */
export function hasLanguageIcon(name: string): boolean {
	return cachedCheck?.(name) === true;
}

/** Returns true once the icon map chunk has been loaded. */
export function isIconMapLoaded(): boolean {
	return cachedResolve !== null;
}

/** Start loading the icon map chunk. Returns a promise that resolves when ready. */
export function loadLanguageIcons(): Promise<void> {
	return ensureLoaded();
}

interface LanguageIconProps {
	name: string;
	size?: string | number;
	className?: string;
}

export function LanguageIcon({ name, size, className }: LanguageIconProps) {
	const [IconComponent, setIconComponent] = useState<DevIconComponent | null>(
		() => cachedResolve?.(name) ?? null,
	);

	useEffect(() => {
		const resolved = cachedResolve?.(name) ?? null;
		if (resolved) {
			setIconComponent(() => resolved);
			return;
		}

		let cancelled = false;

		void ensureLoaded().then(() => {
			if (cancelled) return;
			const loaded = cachedResolve?.(name) ?? null;
			setIconComponent(() => loaded);
		});

		return () => {
			cancelled = true;
		};
	}, [name]);

	if (!IconComponent) return null;
	const resolvedSize = size ?? (className ? "100%" : "1em");
	return (
		<span
			className={cn(
				"inline-flex shrink-0 items-center justify-center",
				className,
			)}
			style={size ? { width: size, height: size } : undefined}
		>
			<IconComponent size={resolvedSize} />
		</span>
	);
}
