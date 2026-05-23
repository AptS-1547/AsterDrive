import { useBlobUrl } from "@/hooks/useBlobUrl";
import { cn } from "@/lib/utils";
import type { AvatarInfo } from "@/types/api";

interface UserAvatarImageProps {
	avatar?: AvatarInfo | null;
	name: string;
	alt?: string;
	className?: string;
	size?: "sm" | "md" | "lg" | "xl";
}

const SIZE_CLASS_MAP = {
	sm: "size-8 text-xs",
	md: "size-10 text-sm",
	lg: "h-24 w-24 text-2xl",
	xl: "h-full w-full text-5xl",
} as const;

function getInitials(name: string) {
	const words = name.trim().split(/\s+/).filter(Boolean);
	if (words.length === 0) {
		return "?";
	}
	if (words.length === 1) {
		return words[0].slice(0, 2).toUpperCase();
	}
	return `${words[0][0] ?? ""}${words[1][0] ?? ""}`.toUpperCase();
}

export function UserAvatarImage({
	avatar,
	name,
	alt,
	className,
	size = "md",
}: UserAvatarImageProps) {
	const preferredUrl =
		size === "xl" || size === "lg"
			? (avatar?.url_1024 ?? avatar?.url_512 ?? null)
			: (avatar?.url_512 ?? avatar?.url_1024 ?? null);
	const internalPath =
		preferredUrl && !/^https?:\/\//i.test(preferredUrl) ? preferredUrl : null;
	const directUrl = internalPath ? null : preferredUrl;
	const { blobUrl, error } = useBlobUrl(internalPath);
	const resolvedSrc = internalPath ? (error ? null : blobUrl) : directUrl;

	return (
		<div
			className={cn(
				"flex shrink-0 items-center justify-center overflow-hidden rounded-2xl bg-muted/60 font-medium text-muted-foreground",
				SIZE_CLASS_MAP[size],
				className,
			)}
		>
			{resolvedSrc ? (
				<img
					src={resolvedSrc}
					alt={alt ?? name}
					loading="lazy"
					decoding="async"
					className="h-full w-full object-cover"
				/>
			) : (
				<span aria-hidden>{getInitials(name)}</span>
			)}
		</div>
	);
}
