import {
	Fragment,
	type ReactNode,
	useCallback,
	useEffect,
	useLayoutEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { useTranslation } from "react-i18next";
import { PreviewAppIcon } from "@/components/common/PreviewAppIcon";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Icon } from "@/components/ui/icon";
import { Input } from "@/components/ui/input";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import { pickLocalizedLabel } from "@/lib/localizedLabel";
import {
	getTablePreviewDelimiterLabelKey,
	normalizeTablePreviewDelimiter,
	type TablePreviewDelimiterValue,
} from "@/lib/tablePreview";
import { cn } from "@/lib/utils";
import {
	createPreviewAppDraft,
	createPreviewRuleDraft,
	formatPreviewAppsDelimitedInput,
	getPreviewAppDefaultIcon,
	getPreviewAppKindLabelKey,
	getPreviewAppProvider,
	getPreviewAppsConfigIssues,
	isExternalPreviewAppKey,
	isProtectedBuiltinPreviewAppKey,
	isTablePreviewAppKey,
	isUrlTemplatePreviewApp,
	isWopiPreviewApp,
	movePreviewEditorItem,
	PREVIEW_APPS_CONFIG_VERSION,
	type PreviewAppProviderValue,
	type PreviewAppsEditorApp,
	type PreviewAppsEditorConfig,
	type PreviewAppsEditorRule,
	parsePreviewAppsConfig,
	parsePreviewAppsDelimitedInput,
	removePreviewRuleAppKey,
	renamePreviewRuleAppKey,
	serializePreviewAppsConfig,
} from "./previewAppsConfigEditorShared";

interface PreviewAppsConfigEditorProps {
	onChange: (value: string) => void;
	value: string;
}

type Translate = (
	key: string,
	values?: Record<string, number | string>,
) => string;

type UrlTemplateMagicVariable = {
	descriptionKey: string;
	labelKey: string;
	token: string;
};

type RuleMatchGroup = {
	key: string;
	label: string;
	values: string[];
};

type RuleMatchSummaryItem = {
	key: string;
	label: string;
	text: string;
};

type SelectedRuleApp = {
	app: PreviewAppsEditorApp;
	index: number;
	key: string;
};

const DETAIL_EXPAND_DURATION_MS = 280;
const DETAIL_COLLAPSE_DURATION_MS = 240;
const DETAIL_EXPAND_EASING = "cubic-bezier(0.22, 1, 0.36, 1)";
const DETAIL_COLLAPSE_EASING = "cubic-bezier(0.32, 0, 0.67, 0.96)";
const RULE_APP_SUMMARY_LIMIT = 2;
const RULE_MATCH_SUMMARY_VALUE_LIMIT = 2;
const URL_TEMPLATE_MAGIC_VARIABLES: UrlTemplateMagicVariable[] = [
	{
		token: "{{file_id}}",
		labelKey: "preview_apps_url_template_variable_file_id_label",
		descriptionKey: "preview_apps_url_template_variable_file_id_desc",
	},
	{
		token: "{{file_name}}",
		labelKey: "preview_apps_url_template_variable_file_name_label",
		descriptionKey: "preview_apps_url_template_variable_file_name_desc",
	},
	{
		token: "{{mime_type}}",
		labelKey: "preview_apps_url_template_variable_mime_type_label",
		descriptionKey: "preview_apps_url_template_variable_mime_type_desc",
	},
	{
		token: "{{size}}",
		labelKey: "preview_apps_url_template_variable_size_label",
		descriptionKey: "preview_apps_url_template_variable_size_desc",
	},
	{
		token: "{{download_path}}",
		labelKey: "preview_apps_url_template_variable_download_path_label",
		descriptionKey: "preview_apps_url_template_variable_download_path_desc",
	},
	{
		token: "{{download_url}}",
		labelKey: "preview_apps_url_template_variable_download_url_label",
		descriptionKey: "preview_apps_url_template_variable_download_url_desc",
	},
	{
		token: "{{file_preview_url}}",
		labelKey: "preview_apps_url_template_variable_file_preview_url_label",
		descriptionKey: "preview_apps_url_template_variable_file_preview_url_desc",
	},
];

function getProviderDefaultIcon(
	key: string,
	provider?: PreviewAppProviderValue | null,
) {
	return getPreviewAppDefaultIcon(key, provider);
}

function getProviderLabel(
	key: string,
	provider: PreviewAppProviderValue | undefined,
	t: Translate,
) {
	return t(getPreviewAppKindLabelKey(key, provider));
}

function getTranslatedLegacyAppLabel(app: PreviewAppsEditorApp, t: Translate) {
	const key = app.label_i18n_key.trim();
	if (!key) {
		return "";
	}

	const translated = t(`files:${key}`);
	if (!translated || translated === key || translated === `files:${key}`) {
		return "";
	}

	return translated;
}

function getLocalizedAppLabel(
	app: PreviewAppsEditorApp,
	language: string | undefined,
	t: Translate,
) {
	return (
		pickLocalizedLabel(app.labels, language) ||
		getTranslatedLegacyAppLabel(app, t) ||
		app.key.trim()
	);
}

function getAppHeading(
	app: PreviewAppsEditorApp,
	index: number,
	language: string | undefined,
	t: Translate,
) {
	return (
		getLocalizedAppLabel(app, language, t) ||
		t("preview_apps_app_title", { index: index + 1 })
	);
}

function isInternalPreviewApp(app: PreviewAppsEditorApp) {
	return !isExternalPreviewAppKey(app.key);
}

function getTablePreviewDelimiterLabel(
	delimiter: TablePreviewDelimiterValue,
	t: Translate,
) {
	return t(getTablePreviewDelimiterLabelKey(delimiter));
}

function getAppSummary(app: PreviewAppsEditorApp, t: Translate) {
	if (isTablePreviewAppKey(app.key)) {
		const delimiter = normalizeTablePreviewDelimiter(app.config.delimiter);
		return `${t("preview_apps_table_delimiter")}: ${getTablePreviewDelimiterLabel(delimiter, t)}`;
	}

	if (isUrlTemplatePreviewApp(app)) {
		const mode =
			typeof app.config.mode === "string" && app.config.mode === "new_tab"
				? t("preview_apps_url_template_mode_new_tab")
				: t("preview_apps_url_template_mode_iframe");
		const urlTemplate =
			typeof app.config.url_template === "string"
				? app.config.url_template.trim()
				: "";
		return urlTemplate ? `${mode} · ${urlTemplate}` : mode;
	}

	if (isWopiPreviewApp(app)) {
		const mode =
			typeof app.config.mode === "string" && app.config.mode === "new_tab"
				? t("preview_apps_wopi_mode_new_tab")
				: t("preview_apps_wopi_mode_iframe");
		const actionUrl =
			typeof app.config.action_url === "string"
				? app.config.action_url.trim()
				: "";
		const discoveryUrl =
			typeof app.config.discovery_url === "string"
				? app.config.discovery_url.trim()
				: "";
		const target = actionUrl || discoveryUrl;
		return target
			? `${t("preview_apps_provider_wopi")} · ${mode} · ${target}`
			: `${t("preview_apps_provider_wopi")} · ${mode}`;
	}

	return getProviderLabel(app.key, app.provider, t);
}

function getRuleMatchGroups(
	rule: PreviewAppsEditorRule,
	t: Translate,
): RuleMatchGroup[] {
	return [
		{
			key: "categories",
			label: t("preview_apps_matches_categories"),
			values: rule.matches.categories,
		},
		{
			key: "extensions",
			label: t("preview_apps_matches_extensions"),
			values: rule.matches.extensions,
		},
		{
			key: "mime_types",
			label: t("preview_apps_matches_mime_types"),
			values: rule.matches.mime_types,
		},
		{
			key: "mime_prefixes",
			label: t("preview_apps_matches_mime_prefixes"),
			values: rule.matches.mime_prefixes,
		},
	].filter((group) => group.values.length > 0);
}

function formatRuleMatchSummaryValue(values: string[]) {
	const visibleValues = values.slice(0, RULE_MATCH_SUMMARY_VALUE_LIMIT);
	const summary = visibleValues.join(", ");
	if (values.length <= RULE_MATCH_SUMMARY_VALUE_LIMIT) {
		return summary;
	}

	return `${summary} +${values.length - RULE_MATCH_SUMMARY_VALUE_LIMIT}`;
}

function getRuleMatchSummaryItems(
	rule: PreviewAppsEditorRule,
	t: Translate,
): RuleMatchSummaryItem[] {
	return getRuleMatchGroups(rule, t).map((group) => ({
		key: group.key,
		label: group.label,
		text: formatRuleMatchSummaryValue(group.values),
	}));
}

function getSelectedRuleApps(
	rule: PreviewAppsEditorRule,
	apps: PreviewAppsEditorApp[],
): SelectedRuleApp[] {
	return apps.flatMap((app, index) => {
		const key = app.key.trim();
		if (!key || !rule.apps.includes(key)) {
			return [];
		}

		return [{ app, index, key }];
	});
}

function formatRuleAppSummary(
	selectedApps: SelectedRuleApp[],
	language: string | undefined,
	t: Translate,
) {
	if (selectedApps.length === 0) {
		return t("preview_apps_no_selected_apps");
	}

	const names = selectedApps
		.slice(0, RULE_APP_SUMMARY_LIMIT)
		.map((selectedApp) =>
			getAppHeading(selectedApp.app, selectedApp.index, language, t),
		);
	const summary = names.join(", ");

	if (selectedApps.length <= RULE_APP_SUMMARY_LIMIT) {
		return summary;
	}

	return `${summary} +${selectedApps.length - RULE_APP_SUMMARY_LIMIT}`;
}

function normalizeRuleDefault(
	rule: PreviewAppsEditorRule,
): PreviewAppsEditorRule {
	if (!rule.apps.includes(rule.default_app)) {
		return {
			...rule,
			default_app: rule.apps[0] ?? "",
		};
	}

	return rule;
}

function moveExpandedIndex(
	current: number | null,
	index: number,
	direction: -1 | 1,
	itemCount: number,
) {
	if (current === null) {
		return null;
	}

	const targetIndex = index + direction;
	if (targetIndex < 0 || targetIndex >= itemCount) {
		return current;
	}

	if (current === index) {
		return targetIndex;
	}

	if (current === targetIndex) {
		return index;
	}

	return current;
}

function EditorField({
	children,
	className,
	description,
	label,
}: {
	children: ReactNode;
	className?: string;
	description?: ReactNode;
	label: string;
}) {
	return (
		<div className={cn("space-y-1.5", className)}>
			<p className="text-xs font-medium text-muted-foreground">{label}</p>
			{children}
			{description ? (
				<div className="text-xs text-muted-foreground">{description}</div>
			) : null}
		</div>
	);
}

function AnimatedCollapsible({
	children,
	className,
	contentClassName,
	open,
}: {
	children: ReactNode;
	className?: string;
	contentClassName?: string;
	open: boolean;
}) {
	const containerRef = useRef<HTMLDivElement | null>(null);
	const contentRef = useRef<HTMLDivElement | null>(null);
	const [mounted, setMounted] = useState(open);

	useEffect(() => {
		if (typeof window === "undefined") {
			setMounted(open);
			return;
		}

		if (open) {
			setMounted(true);
		}
	}, [open]);

	useLayoutEffect(() => {
		if (typeof window === "undefined" || !mounted) {
			return;
		}

		const container = containerRef.current;
		const content = contentRef.current;
		if (!container || !content) {
			return;
		}

		const prefersReducedMotion =
			typeof window.matchMedia === "function" &&
			window.matchMedia("(prefers-reduced-motion: reduce)").matches;
		const duration = prefersReducedMotion
			? 0
			: open
				? DETAIL_EXPAND_DURATION_MS
				: DETAIL_COLLAPSE_DURATION_MS;
		let frameA: number | null = null;
		let frameB: number | null = null;
		let timer: number | null = null;
		const fullHeight = `${content.scrollHeight}px`;

		container.style.overflow = "hidden";
		container.style.transitionProperty = "max-height, opacity";
		container.style.transitionDuration = `${duration}ms`;
		container.style.transitionTimingFunction = open
			? DETAIL_EXPAND_EASING
			: DETAIL_COLLAPSE_EASING;

		if (open) {
			container.style.maxHeight = "0px";
			container.style.opacity = "0";
			frameA = window.requestAnimationFrame(() => {
				frameB = window.requestAnimationFrame(() => {
					container.style.maxHeight = fullHeight;
					container.style.opacity = "1";
				});
			});
			timer = window.setTimeout(() => {
				container.style.maxHeight = "none";
				container.style.opacity = "1";
			}, duration);
		} else {
			container.style.maxHeight = fullHeight;
			container.style.opacity = "1";
			frameA = window.requestAnimationFrame(() => {
				container.style.maxHeight = "0px";
				container.style.opacity = "0";
			});
			timer = window.setTimeout(() => {
				setMounted(false);
			}, duration);
		}

		return () => {
			if (frameA !== null) {
				window.cancelAnimationFrame(frameA);
			}
			if (frameB !== null) {
				window.cancelAnimationFrame(frameB);
			}
			if (timer !== null) {
				window.clearTimeout(timer);
			}
		};
	}, [mounted, open]);

	if (!mounted) {
		return null;
	}

	return (
		<div
			ref={containerRef}
			aria-hidden={!open}
			className={cn("overflow-hidden", className)}
		>
			<div ref={contentRef} className={cn("min-h-0", contentClassName)}>
				{children}
			</div>
		</div>
	);
}

export function PreviewAppsConfigEditor({
	onChange,
	value,
}: PreviewAppsConfigEditorProps) {
	const { i18n, t } = useTranslation(["admin", "files"]);
	const [expandedAppIndex, setExpandedAppIndex] = useState<number | null>(null);
	const [expandedRuleIndex, setExpandedRuleIndex] = useState<number | null>(
		null,
	);
	const [
		activeUrlTemplateVariableAppIndex,
		setActiveUrlTemplateVariableAppIndex,
	] = useState<number | null>(null);

	const parsed = useMemo(() => {
		try {
			const draft = parsePreviewAppsConfig(value);
			return {
				draft,
				issues: getPreviewAppsConfigIssues(draft),
			};
		} catch {
			return {
				draft: null,
				issues: [{ key: "preview_apps_error_parse" }],
			};
		}
	}, [value]);
	const replaceDraft = useCallback(
		(nextDraft: PreviewAppsEditorConfig) => {
			onChange(serializePreviewAppsConfig(nextDraft));
		},
		[onChange],
	);

	const updateDraft = useCallback(
		(
			updater: (current: PreviewAppsEditorConfig) => PreviewAppsEditorConfig,
		) => {
			if (!parsed.draft) {
				return;
			}

			replaceDraft(updater(parsed.draft));
		},
		[parsed.draft, replaceDraft],
	);

	const recoverDraft = useCallback(() => {
		replaceDraft({
			apps: [createPreviewAppDraft([])],
			rules: [],
			version: PREVIEW_APPS_CONFIG_VERSION,
		});
		setExpandedAppIndex(0);
		setExpandedRuleIndex(null);
	}, [replaceDraft]);

	const addApp = useCallback(() => {
		if (!parsed.draft) {
			return;
		}

		setExpandedAppIndex(parsed.draft.apps.length);
		updateDraft((current) => ({
			...current,
			apps: [
				...current.apps,
				createPreviewAppDraft(current.apps.map((app) => app.key)),
			],
		}));
	}, [parsed.draft, updateDraft]);

	const addRule = useCallback(() => {
		if (!parsed.draft) {
			return;
		}

		setExpandedRuleIndex(parsed.draft.rules.length);
		updateDraft((current) => ({
			...current,
			rules: [...current.rules, createPreviewRuleDraft(current.apps)],
		}));
	}, [parsed.draft, updateDraft]);

	const updateApp = useCallback(
		(
			index: number,
			updater: (app: PreviewAppsEditorApp) => PreviewAppsEditorApp,
		) => {
			updateDraft((current) => ({
				...current,
				apps: current.apps.map((app, appIndex) =>
					appIndex === index ? updater(app) : app,
				),
			}));
		},
		[updateDraft],
	);

	const updateRule = useCallback(
		(
			index: number,
			updater: (rule: PreviewAppsEditorRule) => PreviewAppsEditorRule,
		) => {
			updateDraft((current) => ({
				...current,
				rules: current.rules.map((rule, ruleIndex) =>
					ruleIndex === index ? normalizeRuleDefault(updater(rule)) : rule,
				),
			}));
		},
		[updateDraft],
	);

	useEffect(() => {
		if (!parsed.draft) {
			setExpandedAppIndex(null);
			setExpandedRuleIndex(null);
			setActiveUrlTemplateVariableAppIndex(null);
			return;
		}

		setExpandedAppIndex((current) => {
			if (current === null) {
				return null;
			}
			return current < parsed.draft.apps.length ? current : null;
		});
		setExpandedRuleIndex((current) => {
			if (current === null) {
				return null;
			}
			return current < parsed.draft.rules.length ? current : null;
		});
		setActiveUrlTemplateVariableAppIndex((current) => {
			if (current === null) {
				return null;
			}
			return current < parsed.draft.apps.length ? current : null;
		});
	}, [parsed.draft]);

	if (!parsed.draft) {
		return (
			<div className="space-y-3 rounded-xl border border-destructive/30 bg-destructive/5 p-4">
				<div className="flex items-start gap-3">
					<Icon name="Warning" className="mt-0.5 h-4 w-4 text-destructive" />
					<div className="space-y-1">
						<p className="text-sm font-medium text-destructive">
							{t("preview_apps_validation_error")}
						</p>
						<p className="text-sm text-muted-foreground">
							{t("preview_apps_error_parse")}
						</p>
					</div>
				</div>
				<Button variant="outline" size="sm" onClick={recoverDraft}>
					<Icon name="ArrowCounterClockwise" className="h-4 w-4" />
					{t("preview_apps_recover")}
				</Button>
			</div>
		);
	}

	const draft = parsed.draft;
	const issueKeys = parsed.issues.map(
		(issue, issueIndex) => `${issue.key}::${issueIndex}`,
	);
	const appRowKeys = draft.apps.map((_app, index) => `app-row-${index}`);
	const ruleRowKeys = draft.rules.map((_rule, index) => `rule-row-${index}`);
	const activeUrlTemplateVariableApp =
		activeUrlTemplateVariableAppIndex === null
			? null
			: (draft.apps[activeUrlTemplateVariableAppIndex] ?? null);
	const activeUrlTemplateVariableAppName = activeUrlTemplateVariableApp
		? getAppHeading(
				activeUrlTemplateVariableApp,
				activeUrlTemplateVariableAppIndex ?? 0,
				i18n?.language,
				t,
			)
		: "";

	return (
		<>
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
						<span>·</span>
						<span>
							{t("preview_apps_rule_count", { count: draft.rules.length })}
						</span>
					</div>
				</div>

				{parsed.issues.length > 0 ? (
					<div className="space-y-2 rounded-xl border border-destructive/30 bg-destructive/5 p-4">
						<p className="text-sm font-medium text-destructive">
							{t("preview_apps_validation_error")}
						</p>
						<ul className="space-y-1 text-sm text-destructive">
							{parsed.issues.map((issue, issueIndex) => (
								<li key={issueKeys[issueIndex] ?? issue.key}>
									• {t(issue.key, issue.values)}
								</li>
							))}
						</ul>
					</div>
				) : null}

				<section className="space-y-4">
					<div className="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
						<div className="space-y-1">
							<h4 className="text-sm font-semibold">
								{t("preview_apps_apps_section")}
							</h4>
							<p className="text-sm text-muted-foreground">
								{t("preview_apps_apps_section_desc")}
							</p>
						</div>
						<Button variant="outline" size="sm" onClick={addApp}>
							<Icon name="Plus" className="h-4 w-4" />
							{t("preview_apps_add_app")}
						</Button>
					</div>

					{draft.apps.length === 0 ? (
						<p className="text-sm text-muted-foreground">
							{t("preview_apps_no_apps")}
						</p>
					) : (
						<div className="overflow-hidden rounded-2xl border border-border/60 bg-background">
							<Table>
								<TableHeader>
									<TableRow>
										<TableHead className="w-16">
											{t("preview_apps_icon_label")}
										</TableHead>
										<TableHead>{t("preview_apps_column_app")}</TableHead>
										<TableHead>{t("preview_apps_column_summary")}</TableHead>
										<TableHead className="w-24">
											{t("preview_apps_enabled")}
										</TableHead>
										<TableHead className="w-48">{t("core:actions")}</TableHead>
									</TableRow>
								</TableHeader>
								<TableBody>
									{draft.apps.map((app, index) => {
										const rowExpanded = expandedAppIndex === index;
										const rowKey = appRowKeys[index] ?? app.key;
										const internalApp = isInternalPreviewApp(app);
										const protectedBuiltin = isProtectedBuiltinPreviewAppKey(
											app.key,
										);
										const appHeading = getAppHeading(
											app,
											index,
											i18n?.language,
											t,
										);

										return (
											<Fragment key={rowKey}>
												<TableRow
													className={cn(rowExpanded ? "bg-muted/20" : "")}
												>
													<TableCell>
														<div className="flex size-9 items-center justify-center rounded-xl border border-border/50 bg-muted/25">
															<PreviewAppIcon
																icon={app.icon}
																fallback={getProviderDefaultIcon(
																	app.key,
																	app.provider,
																)}
																className="h-4 w-4"
															/>
														</div>
													</TableCell>
													<TableCell className="whitespace-normal">
														<div className="space-y-1">
															<div className="flex flex-wrap items-center gap-2">
																<span className="font-medium">
																	{appHeading}
																</span>
																<Badge variant="outline">
																	{internalApp
																		? t("preview_apps_internal_badge")
																		: t("preview_apps_external_badge")}
																</Badge>
															</div>
														</div>
													</TableCell>
													<TableCell className="whitespace-normal">
														<p className="line-clamp-2 text-sm text-muted-foreground break-all">
															{getAppSummary(app, t)}
														</p>
													</TableCell>
													<TableCell>
														<div className="flex items-center gap-2">
															<Switch
																size="sm"
																checked={app.enabled}
																onCheckedChange={(enabled) =>
																	updateApp(index, (current) => ({
																		...current,
																		enabled,
																	}))
																}
															/>
															<span className="text-xs text-muted-foreground">
																{app.enabled
																	? t("preview_apps_enabled")
																	: t("preview_apps_disabled")}
															</span>
														</div>
													</TableCell>
													<TableCell>
														<div className="flex items-center justify-end gap-1">
															<Button
																variant="ghost"
																size="icon-sm"
																disabled={index === 0}
																aria-label={t("preview_apps_move_up")}
																onClick={() => {
																	setExpandedAppIndex((current) =>
																		moveExpandedIndex(
																			current,
																			index,
																			-1,
																			draft.apps.length,
																		),
																	);
																	updateDraft((current) => ({
																		...current,
																		apps: movePreviewEditorItem(
																			current.apps,
																			index,
																			-1,
																		),
																	}));
																}}
															>
																<Icon name="ArrowUp" className="h-4 w-4" />
															</Button>
															<Button
																variant="ghost"
																size="icon-sm"
																disabled={index === draft.apps.length - 1}
																aria-label={t("preview_apps_move_down")}
																onClick={() => {
																	setExpandedAppIndex((current) =>
																		moveExpandedIndex(
																			current,
																			index,
																			1,
																			draft.apps.length,
																		),
																	);
																	updateDraft((current) => ({
																		...current,
																		apps: movePreviewEditorItem(
																			current.apps,
																			index,
																			1,
																		),
																	}));
																}}
															>
																<Icon name="ArrowDown" className="h-4 w-4" />
															</Button>
															<Button
																variant="ghost"
																size="icon-sm"
																aria-label={
																	rowExpanded
																		? t("preview_apps_collapse")
																		: t("preview_apps_expand")
																}
																onClick={() =>
																	setExpandedAppIndex((current) =>
																		current === index ? null : index,
																	)
																}
															>
																<Icon
																	name="CaretDown"
																	className={cn(
																		"h-4 w-4 transition-transform duration-200 ease-out motion-reduce:transition-none",
																		rowExpanded ? "rotate-180" : "",
																	)}
																/>
															</Button>
															<Button
																variant="ghost"
																size="icon-sm"
																className="text-destructive"
																disabled={protectedBuiltin}
																aria-label={
																	protectedBuiltin
																		? t("preview_apps_builtin_delete_disabled")
																		: t("core:delete")
																}
																onClick={() => {
																	if (protectedBuiltin) {
																		return;
																	}
																	setExpandedAppIndex((current) => {
																		if (current === null) {
																			return null;
																		}
																		if (current === index) {
																			return null;
																		}
																		return current > index
																			? current - 1
																			: current;
																	});
																	updateDraft((current) => {
																		const removedKey =
																			current.apps[index]?.key ?? "";
																		return {
																			...current,
																			apps: current.apps.filter(
																				(_app, appIndex) => appIndex !== index,
																			),
																			rules: removedKey
																				? removePreviewRuleAppKey(
																						current.rules,
																						removedKey,
																					)
																				: current.rules,
																		};
																	});
																}}
															>
																<Icon name="Trash" className="h-4 w-4" />
															</Button>
														</div>
													</TableCell>
												</TableRow>
												<TableRow className="border-b-0 hover:bg-transparent">
													<TableCell
														colSpan={5}
														className="whitespace-normal bg-muted/10 p-0 first:pl-0 last:pr-0 md:first:pl-0 md:last:pr-0"
													>
														<AnimatedCollapsible
															open={rowExpanded}
															contentClassName="px-4 py-3 md:px-6"
														>
															<div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
																<EditorField
																	label={t("preview_apps_key_label")}
																>
																	<Input
																		disabled={protectedBuiltin}
																		value={app.key}
																		onChange={(event) => {
																			const nextKey = event.target.value;
																			updateDraft((current) => {
																				const previousKey =
																					current.apps[index]?.key ?? "";
																				return {
																					...current,
																					apps: current.apps.map(
																						(candidate, appIndex) =>
																							appIndex === index
																								? { ...candidate, key: nextKey }
																								: candidate,
																					),
																					rules:
																						previousKey &&
																						previousKey !== nextKey
																							? renamePreviewRuleAppKey(
																									current.rules,
																									previousKey,
																									nextKey,
																								)
																							: current.rules,
																				};
																			});
																		}}
																	/>
																	{protectedBuiltin ? (
																		<p className="text-xs text-muted-foreground">
																			{t("preview_apps_builtin_key_locked")}
																		</p>
																	) : null}
																</EditorField>
																<EditorField
																	label={t("preview_apps_icon_label")}
																	description={t("preview_apps_icon_hint")}
																>
																	<Input
																		value={app.icon}
																		onChange={(event) =>
																			updateApp(index, (current) => ({
																				...current,
																				icon: event.target.value,
																			}))
																		}
																	/>
																</EditorField>
																{!protectedBuiltin ? (
																	<EditorField
																		label={t("preview_apps_provider_label")}
																	>
																		<Select
																			items={[
																				{
																					label: t(
																						"preview_apps_provider_url_template",
																					),
																					value: "url_template",
																				},
																				{
																					label: t(
																						"preview_apps_provider_wopi",
																					),
																					value: "wopi",
																				},
																			]}
																			value={
																				getPreviewAppProvider(app.provider) ||
																				"url_template"
																			}
																			onValueChange={(provider) =>
																				updateApp(index, (current) => ({
																					...current,
																					provider:
																						provider === "wopi"
																							? "wopi"
																							: "url_template",
																					config: {
																						...current.config,
																						mode:
																							typeof current.config.mode ===
																							"string"
																								? current.config.mode
																								: "iframe",
																					},
																				}))
																			}
																		>
																			<SelectTrigger
																				size="sm"
																				aria-label={t(
																					"preview_apps_provider_label",
																				)}
																			>
																				<SelectValue />
																			</SelectTrigger>
																			<SelectContent>
																				<SelectItem value="url_template">
																					{t(
																						"preview_apps_provider_url_template",
																					)}
																				</SelectItem>
																				<SelectItem value="wopi">
																					{t("preview_apps_provider_wopi")}
																				</SelectItem>
																			</SelectContent>
																		</Select>
																	</EditorField>
																) : null}
																<EditorField
																	label={t("preview_apps_label_zh_label")}
																>
																	<Input
																		value={app.labels.zh ?? ""}
																		onChange={(event) =>
																			updateApp(index, (current) => ({
																				...current,
																				labels: {
																					...current.labels,
																					zh: event.target.value,
																				},
																			}))
																		}
																	/>
																</EditorField>
																<EditorField
																	label={t("preview_apps_label_en_label")}
																>
																	<Input
																		value={app.labels.en ?? ""}
																		onChange={(event) =>
																			updateApp(index, (current) => ({
																				...current,
																				labels: {
																					...current.labels,
																					en: event.target.value,
																				},
																			}))
																		}
																	/>
																</EditorField>
																{isTablePreviewAppKey(app.key) ? (
																	<EditorField
																		label={t("preview_apps_table_delimiter")}
																	>
																		<Select
																			items={[
																				{
																					label: getTablePreviewDelimiterLabel(
																						"auto",
																						t,
																					),
																					value: "auto",
																				},
																				{
																					label: getTablePreviewDelimiterLabel(
																						",",
																						t,
																					),
																					value: ",",
																				},
																				{
																					label: getTablePreviewDelimiterLabel(
																						"\t",
																						t,
																					),
																					value: "\t",
																				},
																				{
																					label: getTablePreviewDelimiterLabel(
																						";",
																						t,
																					),
																					value: ";",
																				},
																				{
																					label: getTablePreviewDelimiterLabel(
																						"|",
																						t,
																					),
																					value: "|",
																				},
																			]}
																			value={normalizeTablePreviewDelimiter(
																				app.config.delimiter,
																			)}
																			onValueChange={(delimiter) =>
																				updateApp(index, (current) => ({
																					...current,
																					config: {
																						...current.config,
																						delimiter:
																							normalizeTablePreviewDelimiter(
																								delimiter,
																							),
																					},
																				}))
																			}
																		>
																			<SelectTrigger
																				size="sm"
																				aria-label={t(
																					"preview_apps_table_delimiter",
																				)}
																			>
																				<SelectValue />
																			</SelectTrigger>
																			<SelectContent>
																				{(
																					["auto", ",", "\t", ";", "|"] as const
																				).map((delimiter) => (
																					<SelectItem
																						key={delimiter}
																						value={delimiter}
																					>
																						{getTablePreviewDelimiterLabel(
																							delimiter,
																							t,
																						)}
																					</SelectItem>
																				))}
																			</SelectContent>
																		</Select>
																	</EditorField>
																) : null}
																{isUrlTemplatePreviewApp(app) ? (
																	<>
																		<EditorField
																			label={t(
																				"preview_apps_url_template_mode",
																			)}
																		>
																			<Select
																				items={[
																					{
																						label: t(
																							"preview_apps_url_template_mode_iframe",
																						),
																						value: "iframe",
																					},
																					{
																						label: t(
																							"preview_apps_url_template_mode_new_tab",
																						),
																						value: "new_tab",
																					},
																				]}
																				value={
																					typeof app.config.mode === "string"
																						? app.config.mode
																						: "iframe"
																				}
																				onValueChange={(mode) =>
																					updateApp(index, (current) => ({
																						...current,
																						config: {
																							...current.config,
																							mode: mode ?? "iframe",
																						},
																					}))
																				}
																			>
																				<SelectTrigger
																					size="sm"
																					aria-label={t(
																						"preview_apps_url_template_mode",
																					)}
																				>
																					<SelectValue />
																				</SelectTrigger>
																				<SelectContent>
																					<SelectItem value="iframe">
																						{t(
																							"preview_apps_url_template_mode_iframe",
																						)}
																					</SelectItem>
																					<SelectItem value="new_tab">
																						{t(
																							"preview_apps_url_template_mode_new_tab",
																						)}
																					</SelectItem>
																				</SelectContent>
																			</Select>
																		</EditorField>
																		<EditorField
																			className="md:col-span-2 xl:col-span-2"
																			label={t("preview_apps_url_template_url")}
																			description={
																				<div className="space-y-2">
																					<p>
																						{t(
																							"preview_apps_url_template_variables_hint",
																						)}
																					</p>
																					<button
																						type="button"
																						className="w-fit text-left text-primary underline-offset-4 transition-colors hover:text-primary/80 hover:underline"
																						onClick={() =>
																							setActiveUrlTemplateVariableAppIndex(
																								index,
																							)
																						}
																					>
																						{t(
																							"preview_apps_url_template_variables_link",
																						)}
																					</button>
																				</div>
																			}
																		>
																			<Input
																				value={
																					typeof app.config.url_template ===
																					"string"
																						? app.config.url_template
																						: ""
																				}
																				onChange={(event) =>
																					updateApp(index, (current) => ({
																						...current,
																						config: {
																							...current.config,
																							url_template: event.target.value,
																						},
																					}))
																				}
																			/>
																		</EditorField>
																		<EditorField
																			className="md:col-span-2 xl:col-span-3"
																			label={t(
																				"preview_apps_url_template_allowed_origins",
																			)}
																		>
																			<Input
																				value={formatPreviewAppsDelimitedInput(
																					Array.isArray(
																						app.config.allowed_origins,
																					)
																						? app.config.allowed_origins.filter(
																								(value): value is string =>
																									typeof value === "string",
																							)
																						: [],
																				)}
																				onChange={(event) =>
																					updateApp(index, (current) => ({
																						...current,
																						config: {
																							...current.config,
																							allowed_origins:
																								parsePreviewAppsDelimitedInput(
																									event.target.value,
																								),
																						},
																					}))
																				}
																			/>
																		</EditorField>
																	</>
																) : null}
																{isWopiPreviewApp(app) ? (
																	<>
																		<EditorField
																			label={t("preview_apps_wopi_mode")}
																			description={t(
																				"preview_apps_wopi_mode_desc",
																			)}
																		>
																			<Select
																				items={[
																					{
																						label: t(
																							"preview_apps_wopi_mode_iframe",
																						),
																						value: "iframe",
																					},
																					{
																						label: t(
																							"preview_apps_wopi_mode_new_tab",
																						),
																						value: "new_tab",
																					},
																				]}
																				value={
																					typeof app.config.mode === "string"
																						? app.config.mode
																						: "iframe"
																				}
																				onValueChange={(mode) =>
																					updateApp(index, (current) => ({
																						...current,
																						config: {
																							...current.config,
																							mode: mode ?? "iframe",
																						},
																					}))
																				}
																			>
																				<SelectTrigger
																					size="sm"
																					aria-label={t(
																						"preview_apps_wopi_mode",
																					)}
																				>
																					<SelectValue />
																				</SelectTrigger>
																				<SelectContent>
																					<SelectItem value="iframe">
																						{t("preview_apps_wopi_mode_iframe")}
																					</SelectItem>
																					<SelectItem value="new_tab">
																						{t(
																							"preview_apps_wopi_mode_new_tab",
																						)}
																					</SelectItem>
																				</SelectContent>
																			</Select>
																		</EditorField>
																		<EditorField
																			className="md:col-span-2 xl:col-span-2"
																			label={t("preview_apps_wopi_action_url")}
																			description={t(
																				"preview_apps_wopi_action_url_desc",
																			)}
																		>
																			<Input
																				value={
																					typeof app.config.action_url ===
																					"string"
																						? app.config.action_url
																						: ""
																				}
																				onChange={(event) =>
																					updateApp(index, (current) => ({
																						...current,
																						config: {
																							...current.config,
																							action_url: event.target.value,
																						},
																					}))
																				}
																			/>
																		</EditorField>
																		<EditorField
																			className="md:col-span-2 xl:col-span-2"
																			label={t(
																				"preview_apps_wopi_discovery_url",
																			)}
																			description={t(
																				"preview_apps_wopi_discovery_url_desc",
																			)}
																		>
																			<Input
																				value={
																					typeof app.config.discovery_url ===
																					"string"
																						? app.config.discovery_url
																						: ""
																				}
																				onChange={(event) =>
																					updateApp(index, (current) => ({
																						...current,
																						config: {
																							...current.config,
																							discovery_url: event.target.value,
																						},
																					}))
																				}
																			/>
																		</EditorField>
																		<EditorField
																			className="md:col-span-2 xl:col-span-2"
																			label={t("preview_apps_wopi_hint_title")}
																			description={t(
																				"preview_apps_wopi_hint_desc",
																			)}
																		>
																			<div className="rounded-xl border border-border/50 bg-muted/20 px-3 py-2 text-sm text-muted-foreground">
																				{t("preview_apps_wopi_hint_body")}
																			</div>
																		</EditorField>
																	</>
																) : null}
															</div>
														</AnimatedCollapsible>
													</TableCell>
												</TableRow>
											</Fragment>
										);
									})}
								</TableBody>
							</Table>
						</div>
					)}
				</section>

				<section className="space-y-4">
					<div className="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
						<div className="space-y-1">
							<h4 className="text-sm font-semibold">
								{t("preview_apps_rules_section")}
							</h4>
							<p className="text-sm text-muted-foreground">
								{t("preview_apps_rules_section_desc")}
							</p>
						</div>
						<Button variant="outline" size="sm" onClick={addRule}>
							<Icon name="Plus" className="h-4 w-4" />
							{t("preview_apps_add_rule")}
						</Button>
					</div>

					{draft.rules.length === 0 ? (
						<p className="text-sm text-muted-foreground">
							{t("preview_apps_no_rules")}
						</p>
					) : (
						<div className="grid gap-3 xl:grid-cols-2">
							{draft.rules.map((rule, index) => {
								const rowExpanded = expandedRuleIndex === index;
								const rowKey = ruleRowKeys[index] ?? `rule-${index}`;
								const defaultValue = rule.default_app || "__none__";
								const selectedApps = getSelectedRuleApps(rule, draft.apps);
								const defaultApp =
									selectedApps.find(
										(candidate) => candidate.key === rule.default_app.trim(),
									) ?? null;
								const matchSummaryItems = getRuleMatchSummaryItems(rule, t);

								return (
									<div
										key={rowKey}
										className={cn(
											"self-start overflow-hidden rounded-2xl border border-border/60 bg-background transition-colors",
											rowExpanded
												? "border-border bg-muted/10 xl:col-span-2"
												: "",
										)}
									>
										<div className="space-y-3 p-4 md:p-5">
											<div className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
												<div className="flex min-w-0 flex-wrap items-center gap-2">
													<span className="text-sm font-semibold">
														{t("preview_apps_rule_title", {
															index: index + 1,
														})}
													</span>
													{defaultApp ? (
														<Badge variant="secondary">
															{t("preview_apps_rule_default_badge", {
																name: getAppHeading(
																	defaultApp.app,
																	defaultApp.index,
																	i18n?.language,
																	t,
																),
															})}
														</Badge>
													) : (
														<Badge variant="outline">
															{t("preview_apps_rule_default_none_badge")}
														</Badge>
													)}
												</div>
												<div className="flex flex-wrap items-center gap-1 self-start md:justify-end">
													<Button
														variant="ghost"
														size="icon-sm"
														disabled={index === 0}
														aria-label={t("preview_apps_move_up")}
														onClick={() => {
															setExpandedRuleIndex((current) =>
																moveExpandedIndex(
																	current,
																	index,
																	-1,
																	draft.rules.length,
																),
															);
															updateDraft((current) => ({
																...current,
																rules: movePreviewEditorItem(
																	current.rules,
																	index,
																	-1,
																),
															}));
														}}
													>
														<Icon name="ArrowUp" className="h-4 w-4" />
													</Button>
													<Button
														variant="ghost"
														size="icon-sm"
														disabled={index === draft.rules.length - 1}
														aria-label={t("preview_apps_move_down")}
														onClick={() => {
															setExpandedRuleIndex((current) =>
																moveExpandedIndex(
																	current,
																	index,
																	1,
																	draft.rules.length,
																),
															);
															updateDraft((current) => ({
																...current,
																rules: movePreviewEditorItem(
																	current.rules,
																	index,
																	1,
																),
															}));
														}}
													>
														<Icon name="ArrowDown" className="h-4 w-4" />
													</Button>
													<Button
														variant="ghost"
														size="icon-sm"
														aria-label={
															rowExpanded
																? t("preview_apps_collapse")
																: t("preview_apps_expand")
														}
														onClick={() =>
															setExpandedRuleIndex((current) =>
																current === index ? null : index,
															)
														}
													>
														<Icon
															name="CaretDown"
															className={cn(
																"h-4 w-4 transition-transform duration-200 ease-out motion-reduce:transition-none",
																rowExpanded ? "rotate-180" : "",
															)}
														/>
													</Button>
													<Button
														variant="ghost"
														size="icon-sm"
														className="text-destructive"
														aria-label={t("core:delete")}
														onClick={() => {
															setExpandedRuleIndex((current) => {
																if (current === null) {
																	return null;
																}
																if (current === index) {
																	return null;
																}
																return current > index ? current - 1 : current;
															});
															updateDraft((current) => ({
																...current,
																rules: current.rules.filter(
																	(_rule, ruleIndex) => ruleIndex !== index,
																),
															}));
														}}
													>
														<Icon name="Trash" className="h-4 w-4" />
													</Button>
												</div>
											</div>

											<div className="flex flex-wrap gap-2">
												{matchSummaryItems.length > 0 ? (
													matchSummaryItems.map((item) => (
														<div
															key={item.key}
															className="inline-flex max-w-full items-baseline gap-1.5 rounded-full border border-border/50 bg-muted/20 px-3 py-1.5 text-sm"
														>
															<span className="shrink-0 text-muted-foreground">
																{item.label}
															</span>
															<span className="break-all font-medium text-foreground/90">
																{item.text}
															</span>
														</div>
													))
												) : (
													<Badge variant="outline">
														{t("preview_apps_rule_no_matches")}
													</Badge>
												)}
												<div className="inline-flex max-w-full items-baseline gap-1.5 rounded-full border border-border/50 bg-muted/20 px-3 py-1.5 text-sm">
													<span className="shrink-0 text-muted-foreground">
														{t("preview_apps_rule_apps_short_label")}
													</span>
													<span className="break-all font-medium text-foreground/90">
														{formatRuleAppSummary(
															selectedApps,
															i18n?.language,
															t,
														)}
													</span>
												</div>
											</div>
										</div>

										<AnimatedCollapsible
											open={rowExpanded}
											contentClassName="border-t border-border/50 bg-muted/10 px-4 py-4 md:px-5"
										>
											<div className="grid gap-4 xl:grid-cols-2">
												<div className="rounded-xl border border-border/50 bg-background/80 p-4">
													<div className="space-y-1">
														<p className="text-sm font-medium">
															{t("preview_apps_column_matches")}
														</p>
														<p className="text-xs text-muted-foreground">
															{t("preview_apps_rule_matches_help")}
														</p>
													</div>
													<div className="mt-4 grid gap-4 md:grid-cols-2">
														<EditorField
															label={t("preview_apps_matches_categories")}
															description={t(
																"preview_apps_matches_categories_desc",
															)}
														>
															<Input
																placeholder={t(
																	"preview_apps_matches_categories_placeholder",
																)}
																value={formatPreviewAppsDelimitedInput(
																	rule.matches.categories,
																)}
																onChange={(event) =>
																	updateRule(index, (current) => ({
																		...current,
																		matches: {
																			...current.matches,
																			categories:
																				parsePreviewAppsDelimitedInput(
																					event.target.value,
																				),
																		},
																	}))
																}
															/>
														</EditorField>
														<EditorField
															label={t("preview_apps_matches_extensions")}
														>
															<Input
																placeholder={t(
																	"preview_apps_matches_extensions_placeholder",
																)}
																value={formatPreviewAppsDelimitedInput(
																	rule.matches.extensions,
																)}
																onChange={(event) =>
																	updateRule(index, (current) => ({
																		...current,
																		matches: {
																			...current.matches,
																			extensions:
																				parsePreviewAppsDelimitedInput(
																					event.target.value,
																				),
																		},
																	}))
																}
															/>
														</EditorField>
														<EditorField
															label={t("preview_apps_matches_mime_types")}
														>
															<Input
																placeholder={t(
																	"preview_apps_matches_mime_types_placeholder",
																)}
																value={formatPreviewAppsDelimitedInput(
																	rule.matches.mime_types,
																)}
																onChange={(event) =>
																	updateRule(index, (current) => ({
																		...current,
																		matches: {
																			...current.matches,
																			mime_types:
																				parsePreviewAppsDelimitedInput(
																					event.target.value,
																				),
																		},
																	}))
																}
															/>
														</EditorField>
														<EditorField
															label={t("preview_apps_matches_mime_prefixes")}
														>
															<Input
																placeholder={t(
																	"preview_apps_matches_mime_prefixes_placeholder",
																)}
																value={formatPreviewAppsDelimitedInput(
																	rule.matches.mime_prefixes,
																)}
																onChange={(event) =>
																	updateRule(index, (current) => ({
																		...current,
																		matches: {
																			...current.matches,
																			mime_prefixes:
																				parsePreviewAppsDelimitedInput(
																					event.target.value,
																				),
																		},
																	}))
																}
															/>
														</EditorField>
													</div>
												</div>

												<div className="rounded-xl border border-border/50 bg-background/80 p-4">
													<div className="flex flex-col gap-3 xl:flex-row xl:items-start xl:justify-between">
														<div className="space-y-1">
															<p className="text-sm font-medium">
																{t("preview_apps_rule_apps_label")}
															</p>
															<p className="text-xs text-muted-foreground">
																{t("preview_apps_rule_apps_help")}
															</p>
														</div>
														<EditorField
															className="xl:w-72"
															label={t("preview_apps_rule_default_label")}
														>
															<Select
																items={[
																	{
																		label: t("preview_apps_default_none"),
																		value: "__none__",
																	},
																	...selectedApps.map((selectedApp) => ({
																		label: getAppHeading(
																			selectedApp.app,
																			selectedApp.index,
																			i18n?.language,
																			t,
																		),
																		value: selectedApp.key,
																	})),
																]}
																value={defaultValue}
																onValueChange={(nextValue) =>
																	updateRule(index, (current) => ({
																		...current,
																		default_app:
																			nextValue == null ||
																			nextValue === "__none__"
																				? ""
																				: nextValue,
																	}))
																}
															>
																<SelectTrigger
																	size="sm"
																	className="w-full"
																	aria-label={t(
																		"preview_apps_rule_default_label",
																	)}
																>
																	<SelectValue />
																</SelectTrigger>
																<SelectContent>
																	<SelectItem value="__none__">
																		{t("preview_apps_default_none")}
																	</SelectItem>
																	{selectedApps.map((selectedApp) => (
																		<SelectItem
																			key={selectedApp.key}
																			value={selectedApp.key}
																		>
																			{getAppHeading(
																				selectedApp.app,
																				selectedApp.index,
																				i18n?.language,
																				t,
																			)}
																		</SelectItem>
																	))}
																</SelectContent>
															</Select>
														</EditorField>
													</div>
													<div className="mt-3 flex flex-wrap gap-2">
														{draft.apps.map((app, appIndex) => {
															const appKey = app.key.trim();
															const selected =
																appKey.length > 0 && rule.apps.includes(appKey);

															return (
																<Button
																	key={appRowKeys[appIndex] ?? app.key}
																	type="button"
																	variant="outline"
																	size="sm"
																	className={cn(
																		"max-w-full gap-2 rounded-full px-3",
																		selected
																			? "border-primary/40 bg-primary/5 text-primary hover:bg-primary/10"
																			: "",
																	)}
																	onClick={() => {
																		if (!appKey) {
																			return;
																		}
																		updateRule(index, (current) => {
																			const apps = current.apps.includes(appKey)
																				? current.apps.filter(
																						(candidate) => candidate !== appKey,
																					)
																				: [...current.apps, appKey];
																			return {
																				...current,
																				apps,
																				default_app: apps.includes(
																					current.default_app,
																				)
																					? current.default_app
																					: (apps[0] ?? ""),
																			};
																		});
																	}}
																>
																	<PreviewAppIcon
																		icon={app.icon}
																		fallback={getProviderDefaultIcon(
																			app.key,
																			app.provider,
																		)}
																		className="h-3.5 w-3.5"
																	/>
																	<span className="truncate">
																		{getAppHeading(
																			app,
																			appIndex,
																			i18n?.language,
																			t,
																		)}
																	</span>
																	{selected ? (
																		<Icon
																			name="Check"
																			className="h-3.5 w-3.5"
																		/>
																	) : null}
																</Button>
															);
														})}
													</div>
												</div>
											</div>
										</AnimatedCollapsible>
									</div>
								);
							})}
						</div>
					)}
				</section>
			</div>

			<Dialog
				open={activeUrlTemplateVariableAppIndex !== null}
				onOpenChange={(open) => {
					if (!open) {
						setActiveUrlTemplateVariableAppIndex(null);
					}
				}}
			>
				<DialogContent className="max-w-[calc(100%-1.5rem)] sm:max-w-[min(56rem,calc(100vw-2rem))]">
					<DialogHeader>
						<DialogTitle>
							{t("preview_apps_url_template_variables_title", {
								name: activeUrlTemplateVariableAppName,
							})}
						</DialogTitle>
						<DialogDescription>
							{t("preview_apps_url_template_variables_dialog_desc")}
						</DialogDescription>
					</DialogHeader>
					<div className="max-h-[min(70vh,40rem)] space-y-3 overflow-y-auto py-2 pr-1">
						{URL_TEMPLATE_MAGIC_VARIABLES.map((variable) => (
							<div
								key={variable.token}
								className="rounded-xl border border-border/60 bg-card/40 px-4 py-4"
							>
								<div className="flex flex-wrap items-center gap-2">
									<code className="break-all rounded bg-muted px-2 py-1 font-mono text-xs">
										{variable.token}
									</code>
									<span className="text-sm font-medium">
										{t(variable.labelKey)}
									</span>
								</div>
								<p className="mt-2 break-words text-sm leading-6 text-muted-foreground">
									{t(variable.descriptionKey)}
								</p>
							</div>
						))}
					</div>
					<DialogFooter showCloseButton />
				</DialogContent>
			</Dialog>
		</>
	);
}
