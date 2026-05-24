import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Icon } from "@/components/ui/icon";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { SECURITY_PANES, type SecurityPane } from "./securityPanes";

interface SecurityTabsShellProps {
	activePane: SecurityPane;
	children: ReactNode;
	onActivePaneChange: (pane: SecurityPane) => void;
}

export function SecurityTabsShell({
	activePane,
	children,
	onActivePaneChange,
}: SecurityTabsShellProps) {
	const { t } = useTranslation(["settings"]);
	const activePaneSummary =
		SECURITY_PANES.find((pane) => pane.value === activePane) ??
		SECURITY_PANES[0];
	const activePaneIndex = SECURITY_PANES.findIndex(
		(pane) => pane.value === activePane,
	);

	return (
		<Tabs
			value={activePane}
			onValueChange={(value) => onActivePaneChange(value as SecurityPane)}
			className="gap-4"
		>
			<div className="space-y-2">
				<div className="flex items-center justify-between gap-3 rounded-xl border bg-muted/20 px-3 py-2 sm:hidden">
					<div className="flex min-w-0 items-center gap-2">
						<div className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-background text-foreground shadow-xs">
							<Icon name={activePaneSummary.icon} className="size-4" />
						</div>
						<span className="truncate text-sm font-medium">
							{t(activePaneSummary.labelKey)}
						</span>
					</div>
					<span className="shrink-0 text-xs tabular-nums text-muted-foreground">
						{activePaneIndex + 1}/{SECURITY_PANES.length}
					</span>
				</div>

				<TabsList className="grid !h-auto min-h-11 w-full grid-cols-5 gap-1 rounded-xl p-1 sm:!h-11">
					{SECURITY_PANES.map((pane) => {
						const label = t(pane.labelKey);
						return (
							<TabsTrigger
								key={pane.value}
								value={pane.value}
								aria-label={label}
								title={label}
								className="h-10 min-w-0 px-0 py-0 sm:h-full sm:px-3"
							>
								<Icon name={pane.icon} className="size-4" />
								<span className="hidden truncate sm:inline">{label}</span>
							</TabsTrigger>
						);
					})}
				</TabsList>
			</div>

			{children}
		</Tabs>
	);
}
