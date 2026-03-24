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
		<button
			type="button"
			className={cn(
				"inline-flex size-5 shrink-0 items-center justify-center rounded-md border shadow-sm transition-colors",
				checked
					? "border-primary bg-primary text-primary-foreground hover:border-primary hover:bg-primary/90"
					: "border-muted-foreground/70 bg-background/90 text-transparent hover:border-foreground/30 hover:bg-accent/40",
				className,
			)}
			onClick={(e) => {
				e.stopPropagation();
				onChange();
			}}
			aria-pressed={checked}
			tabIndex={-1}
		>
			{checked && <Icon name="Check" className="h-3.5 w-3.5 text-current" />}
		</button>
	);
}
