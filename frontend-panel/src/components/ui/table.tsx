import type * as React from "react";
import { useContext } from "react";

import { ScrollAreaContext } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";

function Table({ className, ...props }: React.ComponentProps<"table">) {
	const isInsideScrollArea = useContext(ScrollAreaContext);
	const table = (
		<table
			data-slot="table"
			className={cn("w-full caption-bottom text-sm", className)}
			{...props}
		/>
	);

	if (isInsideScrollArea) {
		return table;
	}

	return (
		<div
			data-slot="table-container"
			data-theme-surface="panel"
			className="relative w-full overflow-x-auto"
		>
			{table}
		</div>
	);
}

function TableHeader({ className, ...props }: React.ComponentProps<"thead">) {
	return (
		<thead
			data-slot="table-header"
			className={cn("[&_tr]:border-b", className)}
			{...props}
		/>
	);
}

function TableBody({ className, ...props }: React.ComponentProps<"tbody">) {
	return (
		<tbody
			data-slot="table-body"
			className={cn("[&_tr:last-child]:border-0", className)}
			{...props}
		/>
	);
}

function TableFooter({ className, ...props }: React.ComponentProps<"tfoot">) {
	return (
		<tfoot
			data-slot="table-footer"
			data-theme-surface="panel"
			className={cn(
				"border-t bg-muted/50 font-medium [&>tr]:last:border-b-0",
				className,
			)}
			{...props}
		/>
	);
}

function TableRow({ className, ...props }: React.ComponentProps<"tr">) {
	return (
		<tr
			data-slot="table-row"
			data-theme-surface="panel"
			className={cn(
				"border-b transition-colors hover:bg-muted/50 data-[state=selected]:bg-muted",
				className,
			)}
			{...props}
		/>
	);
}

function TableHead({ className, ...props }: React.ComponentProps<"th">) {
	return (
		<th
			data-slot="table-head"
			data-theme-surface="panel"
			className={cn(
				"sticky top-0 z-10 h-10 bg-background px-2 text-left align-middle font-medium whitespace-nowrap text-foreground first:pl-4 last:pr-4 md:first:pl-6 md:last:pr-6 [&:has([role=checkbox])]:pr-0",
				className,
			)}
			{...props}
		/>
	);
}

function TableCell({ className, ...props }: React.ComponentProps<"td">) {
	return (
		<td
			data-slot="table-cell"
			className={cn(
				"p-2 align-middle whitespace-nowrap first:pl-4 last:pr-4 md:first:pl-6 md:last:pr-6 [&:has([role=checkbox])]:pr-0",
				className,
			)}
			{...props}
		/>
	);
}

function TableCaption({
	className,
	...props
}: React.ComponentProps<"caption">) {
	return (
		<caption
			data-slot="table-caption"
			className={cn("mt-4 text-sm text-muted-foreground", className)}
			{...props}
		/>
	);
}

export {
	Table,
	TableBody,
	TableCaption,
	TableCell,
	TableFooter,
	TableHead,
	TableHeader,
	TableRow,
};
