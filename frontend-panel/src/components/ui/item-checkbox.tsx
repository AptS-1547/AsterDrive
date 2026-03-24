import { Icon } from "@/components/ui/icon";
import { cn } from "@/lib/utils";

interface ItemCheckboxProps {
	checked: boolean;
	onChange: () => void;
	className?: string;
}

export function ItemCheckbox({
	checked,
	onChange,
	className,
}: ItemCheckboxProps) {
	return (
		// biome-ignore lint/a11y/useSemanticElements: custom styled checkbox matching design system
		<div
			className={cn(
				"h-4 w-4 rounded border flex items-center justify-center cursor-pointer",
				checked ? "bg-primary border-primary" : "border-muted-foreground",
				className,
			)}
			onClick={(e) => {
				e.stopPropagation();
				onChange();
			}}
			onKeyDown={() => {}}
			role="checkbox"
			aria-checked={checked}
			tabIndex={-1}
		>
			{checked && (
				<Icon name="Check" className="h-3 w-3 text-primary-foreground" />
			)}
		</div>
	);
}
