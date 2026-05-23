import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import type {
	PreviewAppsEditorConfig,
	PreviewAppsValidationIssue,
} from "../previewAppsConfigEditorShared";

interface PreviewAppsHeaderProps {
	draft: PreviewAppsEditorConfig;
	issueKeys: string[];
	issues: PreviewAppsValidationIssue[];
	onAddApp: () => void;
}

export function PreviewAppsHeader({
	draft,
	issueKeys,
	issues,
	onAddApp,
}: PreviewAppsHeaderProps) {
	const { t } = useTranslation(["admin", "files"]);

	return (
		<div className="space-y-6">
			<div className="flex flex-col gap-3 rounded-2xl border border-border/60 bg-muted/15 p-4 md:flex-row md:items-start md:justify-between">
				<div className="space-y-1">
					<p className="text-sm font-medium">
						{t("preview_apps_editor_title")}
					</p>
					<p className="max-w-3xl text-sm text-muted-foreground">
						{t("preview_apps_editor_hint")}
					</p>
				</div>
				<div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
					<span>
						{t("preview_apps_version_label", { version: draft.version })}
					</span>
					<span>·</span>
					<span>
						{t("preview_apps_app_count", { count: draft.apps.length })}
					</span>
				</div>
			</div>

			{issues.length > 0 ? (
				<div className="space-y-2 rounded-xl border border-destructive/30 bg-destructive/5 p-4">
					<p className="text-sm font-medium text-destructive">
						{t("preview_apps_validation_error")}
					</p>
					<ul className="space-y-1 text-sm text-destructive">
						{issues.map((issue, issueIndex) => (
							<li key={issueKeys[issueIndex] ?? issue.key}>
								• {t(issue.key, issue.values)}
							</li>
						))}
					</ul>
				</div>
			) : null}

			<div className="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
				<div className="space-y-1">
					<h4 className="text-sm font-semibold">
						{t("preview_apps_apps_section")}
					</h4>
					<p className="text-sm text-muted-foreground">
						{t("preview_apps_apps_section_desc")}
					</p>
				</div>
				<Button variant="outline" size="sm" onClick={onAddApp}>
					<Icon name="Plus" className="size-4" />
					{t("preview_apps_add_app")}
				</Button>
			</div>
		</div>
	);
}
