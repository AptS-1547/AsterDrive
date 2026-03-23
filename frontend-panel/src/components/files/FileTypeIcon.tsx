import { getFileTypeInfo } from "@/components/files/preview/file-capabilities";
import { Icon } from "@/components/ui/icon";
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
	const { icon, color } = getFileTypeInfo({
		mime_type: mimeType,
		name: fileName ?? "unknown",
	});
	return <Icon name={icon} className={cn(color, className)} />;
}
