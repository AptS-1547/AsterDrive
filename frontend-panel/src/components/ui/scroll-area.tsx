import { ScrollArea as ScrollAreaPrimitive } from "@base-ui/react/scroll-area";
import { createContext, forwardRef } from "react";

import { cn } from "@/lib/utils";

export const ScrollAreaContext = createContext(false);

const ScrollArea = forwardRef<HTMLDivElement, ScrollAreaPrimitive.Root.Props>(
	function ScrollArea({ className, children, ...props }, ref) {
		return (
			<ScrollAreaContext.Provider value>
				<ScrollAreaPrimitive.Root
					data-slot="scroll-area"
					className={cn("relative min-h-0 overflow-hidden", className)}
					{...props}
				>
					<ScrollAreaPrimitive.Viewport
						ref={ref}
						data-slot="scroll-area-viewport"
						className="size-full rounded-[inherit] transition-[color,box-shadow] outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-1"
					>
						{children}
					</ScrollAreaPrimitive.Viewport>
					<ScrollBar />
					<ScrollAreaPrimitive.Corner />
				</ScrollAreaPrimitive.Root>
			</ScrollAreaContext.Provider>
		);
	},
);

function ScrollBar({
	className,
	orientation = "vertical",
	...props
}: ScrollAreaPrimitive.Scrollbar.Props) {
	return (
		<ScrollAreaPrimitive.Scrollbar
			data-slot="scroll-area-scrollbar"
			data-theme-surface="chrome"
			data-orientation={orientation}
			orientation={orientation}
			className={cn(
				"flex touch-none p-px transition-colors select-none data-horizontal:h-2.5 data-horizontal:flex-col data-horizontal:border-t data-horizontal:border-t-transparent data-vertical:h-full data-vertical:w-2.5 data-vertical:border-l data-vertical:border-l-transparent",
				className,
			)}
			{...props}
		>
			<ScrollAreaPrimitive.Thumb
				data-slot="scroll-area-thumb"
				className="relative flex-1 rounded-full bg-border"
			/>
		</ScrollAreaPrimitive.Scrollbar>
	);
}

export { ScrollArea, ScrollBar };
