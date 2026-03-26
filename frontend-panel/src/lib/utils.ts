import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
	return twMerge(clsx(inputs));
}

export function sidebarNavItemClass(isActive: boolean, extra?: ClassValue) {
	return cn(
		"flex items-center gap-2 rounded-md px-3 py-2 text-sm transition-colors",
		isActive
			? "bg-accent text-accent-foreground font-medium"
			: "text-muted-foreground hover:bg-accent/50 hover:text-foreground",
		extra,
	);
}
