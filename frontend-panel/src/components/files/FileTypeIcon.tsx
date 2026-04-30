import { useEffect, useState } from "react";
import { getFileTypeInfo } from "@/components/files/preview/file-capabilities";
import { Icon } from "@/components/ui/icon";
import {
	hasLanguageIcon,
	isIconMapLoaded,
	LanguageIcon,
	loadLanguageIcons,
} from "@/components/ui/language-icon";
import { cn } from "@/lib/utils";
import type { FileTypeInfo } from "./preview/types";

interface FileTypeIconProps {
	mimeType: string;
	fileName?: string;
	className?: string;
}

const LANGUAGE_ICON_CATEGORIES = new Set<FileTypeInfo["category"]>([
	"csv",
	"json",
	"markdown",
	"text",
	"tsv",
	"xml",
]);

export function FileTypeIcon({
	mimeType,
	fileName,
	className,
}: FileTypeIconProps) {
	const name = fileName ?? "unknown";
	const [loaded, setLoaded] = useState(isIconMapLoaded);

	useEffect(() => {
		if (loaded) return;

		let cancelled = false;

		void loadLanguageIcons().then(() => {
			if (!cancelled) {
				setLoaded(true);
			}
		});

		return () => {
			cancelled = true;
		};
	}, [loaded]);

	const typeInfo = getFileTypeInfo({
		mime_type: mimeType,
		name,
	});

	if (
		LANGUAGE_ICON_CATEGORIES.has(typeInfo.category) &&
		loaded &&
		hasLanguageIcon(name)
	) {
		return <LanguageIcon name={name} className={className} />;
	}

	const { icon, color } = typeInfo;
	return <Icon name={icon} className={cn(color, className)} />;
}
