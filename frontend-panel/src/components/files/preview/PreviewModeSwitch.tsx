import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import { cn } from "@/lib/utils";
import type { OpenWithMode, OpenWithOption } from "./types";

interface PreviewModeSwitchProps {
	options: OpenWithOption[];
	value: OpenWithMode;
	onChange: (value: OpenWithMode) => void;
}

export function PreviewModeSwitch({
	options,
	value,
	onChange,
}: PreviewModeSwitchProps) {
	const { t } = useTranslation("files");

	if (options.length <= 1) return null;

	return (
		<div className="inline-flex items-center rounded-lg border bg-background p-1">
			{options.map((option) => (
				<Button
					key={option.mode}
					variant="ghost"
					size="sm"
					className={cn(
						"h-7 rounded-md px-2.5 text-xs",
						value === option.mode && "bg-accent text-foreground",
					)}
					onClick={() => onChange(option.mode)}
				>
					<Icon name={option.icon} className="mr-1 h-3.5 w-3.5" />
					{option.label ?? t(option.labelKey)}
				</Button>
			))}
		</div>
	);
}
