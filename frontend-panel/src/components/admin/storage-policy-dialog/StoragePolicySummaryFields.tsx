import { Badge } from "@/components/ui/badge";
import { Icon } from "@/components/ui/icon";
import { cn } from "@/lib/utils";
import type {
	StoragePolicyDriverOption,
	Translate,
} from "./StoragePolicyFieldTypes";

export function StorageDriverVisual({
	className,
	option,
}: {
	className?: string;
	option: StoragePolicyDriverOption;
}) {
	return option.iconSrc ? (
		<img
			src={option.iconSrc}
			alt=""
			className={cn(
				"w-auto object-contain",
				option.type === "local" ? "max-h-7" : "max-h-9",
				className,
			)}
		/>
	) : (
		<Icon
			name={option.iconName ?? "Globe"}
			className={cn("size-8 text-amber-600 dark:text-amber-300", className)}
		/>
	);
}

export function PolicySectionIntro({
	description,
	title,
}: {
	description: string;
	title: string;
}) {
	return (
		<div className="mb-5">
			<h3 className="text-base font-semibold text-foreground">{title}</h3>
			<p className="mt-1 text-sm text-muted-foreground">{description}</p>
		</div>
	);
}

export function PolicySummaryCard({
	currentStorageOption,
	description,
	formName,
	items,
	t,
}: {
	currentStorageOption: StoragePolicyDriverOption;
	description: string;
	formName: string;
	items: Array<{ label: string; value: string }>;
	t: Translate;
}) {
	return (
		<div
			data-testid="policy-summary-card"
			className="rounded-3xl border border-border/70 bg-muted/20 p-5"
		>
			<div className="flex items-center gap-3">
				<div className="flex size-14 items-center justify-center rounded-2xl bg-white shadow-sm ring-1 ring-black/5">
					<StorageDriverVisual option={currentStorageOption} />
				</div>
				<div>
					<p className="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
						{t("policy_wizard_summary_title")}
					</p>
					<h3 className="mt-1 text-base font-semibold">
						{formName || t("new_policy")}
					</h3>
				</div>
			</div>
			<p className="mt-4 text-sm leading-6 text-muted-foreground">
				{description}
			</p>
			<div className="mt-4 overflow-hidden rounded-2xl border border-border/70 bg-background/85">
				<dl className="divide-y divide-border/70">
					{items.map((item) => (
						<div
							key={item.label}
							className="grid grid-cols-[96px_minmax(0,1fr)] items-start gap-3 px-4 py-3"
						>
							<dt className="pt-0.5 text-[11px] font-medium uppercase tracking-[0.14em] text-muted-foreground">
								{item.label}
							</dt>
							<dd className="min-w-0 break-all text-sm font-medium leading-5 text-foreground">
								{item.value}
							</dd>
						</div>
					))}
				</dl>
			</div>
		</div>
	);
}

export function DriverTypeBadge({
	className,
	title,
}: {
	className: string;
	title: string;
}) {
	return (
		<Badge
			variant="outline"
			data-testid="policy-driver-badge"
			className={className}
		>
			{title}
		</Badge>
	);
}
