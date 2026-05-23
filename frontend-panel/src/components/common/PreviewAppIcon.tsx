import { useRef } from "react";
import { Icon } from "@/components/ui/icon";
import { cn } from "@/lib/utils";

interface PreviewAppIconProps {
	icon?: string | null;
	fallback?: string | null;
	className?: string;
	alt?: string;
}

const ICON_URL_PATTERN =
	/^(https?:\/\/|\/\/|\/(?!\/)|\.\/|\.\.\/|data:image\/|blob:)/i;

function isPreviewAppIconUrl(value: string) {
	return ICON_URL_PATTERN.test(value.trim());
}

export function PreviewAppIcon({
	icon,
	fallback = "",
	className,
	alt = "",
}: PreviewAppIconProps) {
	const value = icon?.trim() ?? "";
	const fallbackValue = fallback?.trim() ?? "";
	const failedSourcesRef = useRef<Set<string>>(new Set());
	const sourceKey = `${value}\u0000${fallbackValue}`;
	const previousSourceKeyRef = useRef(sourceKey);

	if (previousSourceKeyRef.current !== sourceKey) {
		previousSourceKeyRef.current = sourceKey;
		failedSourcesRef.current = new Set();
	}

	const seenCandidates = new Set<string>();
	const urlCandidates: string[] = [];
	for (const candidate of [value, fallbackValue]) {
		if (
			candidate.length > 0 &&
			isPreviewAppIconUrl(candidate) &&
			!seenCandidates.has(candidate)
		) {
			seenCandidates.add(candidate);
			urlCandidates.push(candidate);
		}
	}

	const firstUrlIndex = urlCandidates.findIndex(
		(candidate) => !failedSourcesRef.current.has(candidate),
	);
	if (firstUrlIndex >= 0) {
		const candidate = urlCandidates[firstUrlIndex];
		return (
			<span
				key={sourceKey}
				className="inline-flex shrink-0 items-center justify-center"
			>
				<img
					src={candidate}
					alt={alt}
					loading="lazy"
					decoding="async"
					data-preview-icon-index={firstUrlIndex}
					className={cn("shrink-0 object-contain", className)}
					onError={(event) => {
						const image = event.currentTarget;
						const currentIndex = Number(
							image.dataset.previewIconIndex ?? firstUrlIndex,
						);
						failedSourcesRef.current.add(urlCandidates[currentIndex]);

						for (
							let nextIndex = currentIndex + 1;
							nextIndex < urlCandidates.length;
							nextIndex += 1
						) {
							const nextCandidate = urlCandidates[nextIndex];
							if (failedSourcesRef.current.has(nextCandidate)) {
								continue;
							}

							image.dataset.previewIconIndex = String(nextIndex);
							image.src = nextCandidate;
							return;
						}

						image.classList.add("hidden");
						image.nextElementSibling?.classList.remove("hidden");
					}}
				/>
				<Icon
					name="File"
					className={cn("hidden", className)}
					aria-hidden={alt ? undefined : true}
					aria-label={alt || undefined}
					role={alt ? "img" : undefined}
				/>
			</span>
		);
	}

	return (
		<Icon
			name="File"
			className={className}
			aria-hidden={alt ? undefined : true}
		/>
	);
}
