import { cva } from "class-variance-authority";

export const tabsListVariants = cva(
	"group/tabs-list items-center justify-center rounded-lg p-[3px] text-muted-foreground group-data-horizontal/tabs:h-8 group-data-vertical/tabs:h-fit group-data-vertical/tabs:flex-col data-[variant=line]:rounded-none",
	{
		variants: {
			variant: {
				default: "inline-flex w-fit bg-muted",
				line: "flex min-w-0 w-full max-w-full gap-1 bg-transparent",
			},
		},
		defaultVariants: {
			variant: "default",
		},
	},
);
