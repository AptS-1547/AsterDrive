import type { ReactNode } from "react";
import {
	Card,
	CardContent,
	CardDescription,
	CardHeader,
	CardTitle,
} from "@/components/ui/card";
import { Icon, type IconName } from "@/components/ui/icon";
import { cn } from "@/lib/utils";

export function SettingsPageIntro({
	title,
	description,
}: {
	title: string;
	description?: string;
}) {
	return (
		<header className="space-y-1">
			<h1 className="text-xl font-semibold tracking-tight">{title}</h1>
			{description ? (
				<p className="text-sm text-muted-foreground">{description}</p>
			) : null}
		</header>
	);
}

export function SettingsSection({
	title,
	description,
	action,
	children,
	className,
	contentClassName,
}: {
	title: string;
	description?: string;
	action?: ReactNode;
	children: ReactNode;
	className?: string;
	contentClassName?: string;
}) {
	return (
		<Card className={cn("gap-0", className)}>
			<CardHeader className="border-b">
				<div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
					<div className="min-w-0">
						<CardTitle>{title}</CardTitle>
						{description ? (
							<CardDescription>{description}</CardDescription>
						) : null}
					</div>
					{action ? (
						<div className="flex items-center gap-2 self-start">{action}</div>
					) : null}
				</div>
			</CardHeader>
			<CardContent className={cn("pt-2", contentClassName)}>
				{children}
			</CardContent>
		</Card>
	);
}

export function SettingsRow({
	label,
	description,
	children,
	className,
	controlClassName,
}: {
	label: string;
	description?: string;
	children: ReactNode;
	className?: string;
	controlClassName?: string;
}) {
	return (
		<div
			className={cn(
				"flex flex-col gap-3 border-b py-4 first:pt-0 last:border-b-0 last:pb-0 md:flex-row md:items-start md:justify-between",
				className,
			)}
		>
			<div className="min-w-0 md:max-w-sm">
				<p className="text-sm font-medium">{label}</p>
				{description ? (
					<p className="mt-1 text-sm text-muted-foreground">{description}</p>
				) : null}
			</div>
			<div className={cn("w-full md:max-w-[360px]", controlClassName)}>
				{children}
			</div>
		</div>
	);
}

export function SettingsChoiceGroup<T extends string>({
	options,
	value,
	onChange,
	className,
}: {
	options: Array<{
		value: T;
		label: string;
		icon?: IconName;
	}>;
	value: T;
	onChange: (value: T) => void;
	className?: string;
}) {
	return (
		<div className={cn("flex flex-wrap gap-2", className)}>
			{options.map((option) => {
				const active = option.value === value;
				return (
					<button
						key={option.value}
						type="button"
						aria-pressed={active}
						onClick={() => onChange(option.value)}
						className={cn(
							"inline-flex items-center gap-2 rounded-lg border px-3 py-2 text-sm font-medium transition-colors",
							active
								? "border-primary/20 bg-primary/10 text-foreground"
								: "border-border bg-background hover:bg-muted/40",
						)}
					>
						{option.icon ? (
							<Icon name={option.icon} className="size-4" />
						) : null}
						<span>{option.label}</span>
					</button>
				);
			})}
		</div>
	);
}
