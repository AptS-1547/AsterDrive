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

interface FileTypeIconProps {
	mimeType: string;
	fileName?: string;
	className?: string;
}

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

	if (loaded && hasLanguageIcon(name)) {
		return <LanguageIcon name={name} className={className} />;
	}

	const { icon, color } = getFileTypeInfo({
		mime_type: mimeType,
		name,
	});
	return <Icon name={icon} className={cn(color, className)} />;
}
