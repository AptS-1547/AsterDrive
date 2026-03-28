import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Icon } from "@/components/ui/icon";
import type { OpenWithMode, OpenWithOption } from "./types";

interface OpenWithChooserProps {
	options: OpenWithOption[];
	value: OpenWithMode;
	onChange: (value: OpenWithMode) => void;
}

export function OpenWithChooser({
	options,
	value,
	onChange,
}: OpenWithChooserProps) {
	const { t } = useTranslation("files");
	const current = options.find((option) => option.mode === value) ?? options[0];

	if (!current || options.length <= 1) {
		return null;
	}

	return (
		<DropdownMenu>
			<DropdownMenuTrigger render={<Button variant="outline" size="sm" />}>
				<Icon name={current.icon} className="mr-1 h-3.5 w-3.5" />
				{current.label ?? t(current.labelKey)}
				<Icon name="CaretDown" className="ml-1 h-3.5 w-3.5" />
			</DropdownMenuTrigger>
			<DropdownMenuContent align="start" className="w-48 min-w-48">
				{options.map((option) => (
					<DropdownMenuItem
						key={option.mode}
						onClick={() => onChange(option.mode)}
					>
						<Icon name={option.icon} className="mr-2 h-4 w-4" />
						{option.label ?? t(option.labelKey)}
						{option.mode === value && (
							<Icon name="Check" className="ml-auto h-4 w-4" />
						)}
					</DropdownMenuItem>
				))}
			</DropdownMenuContent>
		</DropdownMenu>
	);
}
