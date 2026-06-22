import { LoadingSpinner } from "@/components/common/LoadingSpinner";
import { cn } from "@/lib/utils";

interface PreviewLoadingStateProps {
	text: string;
	className?: string;
}

export function PreviewLoadingState({
	text,
	className,
}: PreviewLoadingStateProps) {
	return (
		<LoadingSpinner
			text={text}
			className={cn("min-h-[18rem] w-full py-0", className)}
		/>
	);
}
