import {
	type ReactNode,
	useCallback,
	useEffect,
	useMemo,
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
	formatPreviewAppsDelimitedInput,
	getPreviewAppDefaultIcon,
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
	parsePreviewAppsConfig,
	parsePreviewAppsDelimitedInput,
	serializePreviewAppsConfig,
} from "./previewAppsConfigEditorShared";

interface PreviewAppsConfigEditorProps {
	onBuildWopiDiscoveryPreviewConfig?: (input: {
		discoveryUrl: string;
		value: string;
	}) => Promise<string>;
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
	return getExtensionSummary(app, t);
}

function getExtensionSummary(app: PreviewAppsEditorApp, t: Translate) {
	if (app.extensions.length === 0) {
		return t("preview_apps_extensions_any");
	}

	return formatPreviewAppsDelimitedInput(app.extensions);
}

function moveActiveAppIndex(
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

function PreviewAppEditorFields({
	app,
	index,
	protectedBuiltin,
	t,
	updateApp,
	updateDraft,
	onOpenUrlTemplateVariables,
}: {
	app: PreviewAppsEditorApp;
	index: number;
	protectedBuiltin: boolean;
	t: Translate;
	updateApp: (
		index: number,
		updater: (app: PreviewAppsEditorApp) => PreviewAppsEditorApp,
	) => void;
	updateDraft: (
		updater: (current: PreviewAppsEditorConfig) => PreviewAppsEditorConfig,
	) => void;
	onOpenUrlTemplateVariables: () => void;
}) {
	return (
		<div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
			<EditorField label={t("preview_apps_key_label")}>
				<Input
					disabled={protectedBuiltin}
					value={app.key}
					onChange={(event) => {
						const nextKey = event.target.value;
						updateDraft((current) => {
							return {
								...current,
								apps: current.apps.map((candidate, appIndex) =>
									appIndex === index
										? { ...candidate, key: nextKey }
										: candidate,
								),
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
				<EditorField label={t("preview_apps_provider_label")}>
					<Select
						items={[
							{
								label: t("preview_apps_provider_url_template"),
								value: "url_template",
							},
							{
								label: t("preview_apps_provider_wopi"),
								value: "wopi",
							},
						]}
						value={getPreviewAppProvider(app.provider) || "url_template"}
						onValueChange={(provider) =>
							updateApp(index, (current) => ({
								...current,
								provider: provider === "wopi" ? "wopi" : "url_template",
								config: {
									...current.config,
									mode:
										typeof current.config.mode === "string"
											? current.config.mode
											: "iframe",
								},
							}))
						}
					>
						<SelectTrigger
							size="sm"
							aria-label={t("preview_apps_provider_label")}
						>
							<SelectValue />
						</SelectTrigger>
						<SelectContent>
							<SelectItem value="url_template">
								{t("preview_apps_provider_url_template")}
							</SelectItem>
							<SelectItem value="wopi">
								{t("preview_apps_provider_wopi")}
							</SelectItem>
						</SelectContent>
					</Select>
				</EditorField>
			) : null}
			<EditorField label={t("preview_apps_label_zh_label")}>
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
			<EditorField label={t("preview_apps_label_en_label")}>
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
			<EditorField
				className="md:col-span-2 xl:col-span-2"
				label={t("preview_apps_matches_extensions")}
				description={t("preview_apps_list_input_hint")}
			>
				<Input
					placeholder={t("preview_apps_matches_extensions_placeholder")}
					value={formatPreviewAppsDelimitedInput(app.extensions)}
					onChange={(event) =>
						updateApp(index, (current) => ({
							...current,
							extensions: parsePreviewAppsDelimitedInput(event.target.value),
						}))
					}
				/>
			</EditorField>
			{isTablePreviewAppKey(app.key) ? (
				<EditorField label={t("preview_apps_table_delimiter")}>
					<Select
						items={[
							{
								label: getTablePreviewDelimiterLabel("auto", t),
								value: "auto",
							},
							{
								label: getTablePreviewDelimiterLabel(",", t),
								value: ",",
							},
							{
								label: getTablePreviewDelimiterLabel("\t", t),
								value: "\t",
							},
							{
								label: getTablePreviewDelimiterLabel(";", t),
								value: ";",
							},
							{
								label: getTablePreviewDelimiterLabel("|", t),
								value: "|",
							},
						]}
						value={normalizeTablePreviewDelimiter(app.config.delimiter)}
						onValueChange={(delimiter) =>
							updateApp(index, (current) => ({
								...current,
								config: {
									...current.config,
									delimiter: normalizeTablePreviewDelimiter(delimiter),
								},
							}))
						}
					>
						<SelectTrigger
							size="sm"
							aria-label={t("preview_apps_table_delimiter")}
						>
							<SelectValue />
						</SelectTrigger>
						<SelectContent>
							{(["auto", ",", "\t", ";", "|"] as const).map((delimiter) => (
								<SelectItem key={delimiter} value={delimiter}>
									{getTablePreviewDelimiterLabel(delimiter, t)}
								</SelectItem>
							))}
						</SelectContent>
					</Select>
				</EditorField>
			) : null}
			{isUrlTemplatePreviewApp(app) ? (
				<>
					<EditorField label={t("preview_apps_url_template_mode")}>
						<Select
							items={[
								{
									label: t("preview_apps_url_template_mode_iframe"),
									value: "iframe",
								},
								{
									label: t("preview_apps_url_template_mode_new_tab"),
									value: "new_tab",
								},
							]}
							value={
								typeof app.config.mode === "string" ? app.config.mode : "iframe"
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
								aria-label={t("preview_apps_url_template_mode")}
							>
								<SelectValue />
							</SelectTrigger>
							<SelectContent>
								<SelectItem value="iframe">
									{t("preview_apps_url_template_mode_iframe")}
								</SelectItem>
								<SelectItem value="new_tab">
									{t("preview_apps_url_template_mode_new_tab")}
								</SelectItem>
							</SelectContent>
						</Select>
					</EditorField>
					<EditorField
						className="md:col-span-2 xl:col-span-2"
						label={t("preview_apps_url_template_url")}
						description={
							<div className="space-y-2">
								<p>{t("preview_apps_url_template_variables_hint")}</p>
								<button
									type="button"
									className="w-fit text-left text-primary underline-offset-4 transition-colors hover:text-primary/80 hover:underline"
									onClick={onOpenUrlTemplateVariables}
								>
									{t("preview_apps_url_template_variables_link")}
								</button>
							</div>
						}
					>
						<Input
							value={
								typeof app.config.url_template === "string"
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
						label={t("preview_apps_url_template_allowed_origins")}
					>
						<Input
							value={formatPreviewAppsDelimitedInput(
								Array.isArray(app.config.allowed_origins)
									? app.config.allowed_origins.filter(
											(value): value is string => typeof value === "string",
										)
									: [],
							)}
							onChange={(event) =>
								updateApp(index, (current) => ({
									...current,
									config: {
										...current.config,
										allowed_origins: parsePreviewAppsDelimitedInput(
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
						description={t("preview_apps_wopi_mode_desc")}
					>
						<Select
							items={[
								{
									label: t("preview_apps_wopi_mode_iframe"),
									value: "iframe",
								},
								{
									label: t("preview_apps_wopi_mode_new_tab"),
									value: "new_tab",
								},
							]}
							value={
								typeof app.config.mode === "string" ? app.config.mode : "iframe"
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
							<SelectTrigger size="sm" aria-label={t("preview_apps_wopi_mode")}>
								<SelectValue />
							</SelectTrigger>
							<SelectContent>
								<SelectItem value="iframe">
									{t("preview_apps_wopi_mode_iframe")}
								</SelectItem>
								<SelectItem value="new_tab">
									{t("preview_apps_wopi_mode_new_tab")}
								</SelectItem>
							</SelectContent>
						</Select>
					</EditorField>
					<EditorField
						className="md:col-span-2 xl:col-span-2"
						label={t("preview_apps_wopi_action_url")}
						description={t("preview_apps_wopi_action_url_desc")}
					>
						<Input
							value={
								typeof app.config.action_url === "string"
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
						label={t("preview_apps_wopi_discovery_url")}
						description={t("preview_apps_wopi_discovery_url_desc")}
					>
						<Input
							value={
								typeof app.config.discovery_url === "string"
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
						description={t("preview_apps_wopi_hint_desc")}
					>
						<div className="rounded-xl border border-border/50 bg-muted/20 px-3 py-2 text-sm text-muted-foreground">
							{t("preview_apps_wopi_hint_body")}
						</div>
					</EditorField>
				</>
			) : null}
		</div>
	);
}

export function PreviewAppsConfigEditor({
	onBuildWopiDiscoveryPreviewConfig,
	onChange,
	value,
}: PreviewAppsConfigEditorProps) {
	const { i18n, t } = useTranslation(["admin", "files"]);
	const [addAppDialogOpen, setAddAppDialogOpen] = useState(false);
	const [buildingWopiDiscoveryConfig, setBuildingWopiDiscoveryConfig] =
		useState(false);
	const [editingAppIndex, setEditingAppIndex] = useState<number | null>(null);
	const [
		activeUrlTemplateVariableAppIndex,
		setActiveUrlTemplateVariableAppIndex,
	] = useState<number | null>(null);
	const [wopiDiscoveryDialogOpen, setWopiDiscoveryDialogOpen] = useState(false);
	const [wopiDiscoveryUrl, setWopiDiscoveryUrl] = useState("");

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
			version: PREVIEW_APPS_CONFIG_VERSION,
		});
		setEditingAppIndex(0);
	}, [replaceDraft]);

	const addEmbedApp = useCallback(() => {
		if (!parsed.draft) {
			return;
		}

		setAddAppDialogOpen(false);
		setEditingAppIndex(parsed.draft.apps.length);
		updateDraft((current) => ({
			...current,
			apps: [
				...current.apps,
				createPreviewAppDraft(current.apps.map((app) => app.key)),
			],
		}));
	}, [parsed.draft, updateDraft]);

	const buildWopiDiscoveryConfig = useCallback(async () => {
		if (!onBuildWopiDiscoveryPreviewConfig) {
			return;
		}

		const discoveryUrl = wopiDiscoveryUrl.trim();
		if (!discoveryUrl) {
			return;
		}

		setBuildingWopiDiscoveryConfig(true);
		try {
			const nextValue = await onBuildWopiDiscoveryPreviewConfig({
				discoveryUrl,
				value,
			});
			onChange(nextValue);
			setWopiDiscoveryDialogOpen(false);
			setWopiDiscoveryUrl("");
		} catch {
			// Errors are handled by the caller so the dialog can stay open for retry.
		} finally {
			setBuildingWopiDiscoveryConfig(false);
		}
	}, [onBuildWopiDiscoveryPreviewConfig, onChange, value, wopiDiscoveryUrl]);

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

	useEffect(() => {
		if (!parsed.draft) {
			setAddAppDialogOpen(false);
			setEditingAppIndex(null);
			setActiveUrlTemplateVariableAppIndex(null);
			setWopiDiscoveryDialogOpen(false);
			setWopiDiscoveryUrl("");
			return;
		}

		setEditingAppIndex((current) => {
			if (current === null) {
				return null;
			}
			return current < parsed.draft.apps.length ? current : null;
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
	const activeUrlTemplateVariableApp =
		activeUrlTemplateVariableAppIndex === null
			? null
			: (draft.apps[activeUrlTemplateVariableAppIndex] ?? null);
	const activeEditingApp =
		editingAppIndex === null ? null : (draft.apps[editingAppIndex] ?? null);
	const activeEditingAppName = activeEditingApp
		? getAppHeading(activeEditingApp, editingAppIndex ?? 0, i18n?.language, t)
		: "";
	const activeEditingAppProtectedBuiltin = activeEditingApp
		? isProtectedBuiltinPreviewAppKey(activeEditingApp.key)
		: false;
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
						<Button
							variant="outline"
							size="sm"
							onClick={() => setAddAppDialogOpen(true)}
						>
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
										const rowEditing = editingAppIndex === index;
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
											<TableRow
												key={rowKey}
												className={cn(rowEditing ? "bg-muted/20" : "")}
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
															<span className="font-medium">{appHeading}</span>
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
																setEditingAppIndex((current) =>
																	moveActiveAppIndex(
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
																setEditingAppIndex((current) =>
																	moveActiveAppIndex(
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
															aria-label={t("preview_apps_edit")}
															onClick={() => setEditingAppIndex(index)}
														>
															<Icon name="PencilSimple" className="h-4 w-4" />
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
																setEditingAppIndex((current) => {
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
																	return {
																		...current,
																		apps: current.apps.filter(
																			(_app, appIndex) => appIndex !== index,
																		),
																	};
																});
															}}
														>
															<Icon name="Trash" className="h-4 w-4" />
														</Button>
													</div>
												</TableCell>
											</TableRow>
										);
									})}
								</TableBody>
							</Table>
						</div>
					)}
				</section>
			</div>

			<Dialog
				open={addAppDialogOpen}
				onOpenChange={(open) => {
					setAddAppDialogOpen(open);
				}}
			>
				<DialogContent className="max-w-md">
					<DialogHeader>
						<DialogTitle>{t("preview_apps_add_dialog_title")}</DialogTitle>
						<DialogDescription>
							{t("preview_apps_add_dialog_desc")}
						</DialogDescription>
					</DialogHeader>
					<div className="grid gap-3 py-2">
						<Button
							variant="outline"
							className="h-auto w-full min-w-0 items-start justify-start px-4 py-4 text-left whitespace-normal"
							onClick={addEmbedApp}
						>
							<div className="min-w-0 space-y-1">
								<p className="break-words font-medium">
									{t("preview_apps_add_dialog_embed_title")}
								</p>
								<p className="break-words text-sm text-muted-foreground">
									{t("preview_apps_add_dialog_embed_desc")}
								</p>
							</div>
						</Button>
						{onBuildWopiDiscoveryPreviewConfig ? (
							<Button
								variant="outline"
								className="h-auto w-full min-w-0 items-start justify-start px-4 py-4 text-left whitespace-normal"
								onClick={() => {
									setAddAppDialogOpen(false);
									setWopiDiscoveryDialogOpen(true);
								}}
							>
								<div className="min-w-0 space-y-1">
									<p className="break-words font-medium">
										{t("preview_apps_add_dialog_wopi_title")}
									</p>
									<p className="break-words text-sm text-muted-foreground">
										{t("preview_apps_add_dialog_wopi_desc")}
									</p>
								</div>
							</Button>
						) : null}
					</div>
					<DialogFooter showCloseButton />
				</DialogContent>
			</Dialog>

			<Dialog
				open={wopiDiscoveryDialogOpen}
				onOpenChange={(open) => {
					setWopiDiscoveryDialogOpen(open);
					if (!open) {
						setWopiDiscoveryUrl("");
					}
				}}
			>
				<DialogContent className="max-w-md">
					<DialogHeader>
						<DialogTitle>
							{t("preview_apps_wopi_discovery_dialog_title")}
						</DialogTitle>
						<DialogDescription>
							{t("preview_apps_wopi_discovery_dialog_desc")}
						</DialogDescription>
					</DialogHeader>
					<div className="space-y-2 py-2">
						<p className="text-xs font-medium text-muted-foreground">
							{t("preview_apps_wopi_discovery_dialog_label")}
						</p>
						<Input
							aria-label={t("preview_apps_wopi_discovery_dialog_label")}
							placeholder={t("preview_apps_wopi_discovery_dialog_placeholder")}
							value={wopiDiscoveryUrl}
							onChange={(event) => setWopiDiscoveryUrl(event.target.value)}
						/>
					</div>
					<DialogFooter>
						<Button
							variant="outline"
							onClick={() => {
								setWopiDiscoveryDialogOpen(false);
								setWopiDiscoveryUrl("");
							}}
						>
							{t("core:cancel")}
						</Button>
						<Button
							disabled={
								buildingWopiDiscoveryConfig ||
								wopiDiscoveryUrl.trim().length === 0
							}
							onClick={() => void buildWopiDiscoveryConfig()}
						>
							{buildingWopiDiscoveryConfig
								? t("preview_apps_wopi_discovery_dialog_loading")
								: t("preview_apps_wopi_discovery_dialog_submit")}
						</Button>
					</DialogFooter>
				</DialogContent>
			</Dialog>

			<Dialog
				open={activeEditingApp !== null}
				onOpenChange={(open) => {
					if (!open) {
						setEditingAppIndex(null);
						setActiveUrlTemplateVariableAppIndex(null);
					}
				}}
			>
				<DialogContent className="max-w-[calc(100%-1.5rem)] sm:max-w-[min(72rem,calc(100vw-2rem))]">
					<DialogHeader>
						<DialogTitle>
							{t("preview_apps_dialog_title", {
								name: activeEditingAppName,
							})}
						</DialogTitle>
						<DialogDescription>
							{t("preview_apps_dialog_desc")}
						</DialogDescription>
					</DialogHeader>
					{activeEditingApp ? (
						<div className="max-h-[min(72vh,46rem)] overflow-y-auto py-2 pr-1">
							<PreviewAppEditorFields
								app={activeEditingApp}
								index={editingAppIndex ?? 0}
								protectedBuiltin={activeEditingAppProtectedBuiltin}
								t={t}
								updateApp={updateApp}
								updateDraft={updateDraft}
								onOpenUrlTemplateVariables={() =>
									setActiveUrlTemplateVariableAppIndex(editingAppIndex ?? 0)
								}
							/>
						</div>
					) : null}
					<DialogFooter showCloseButton />
				</DialogContent>
			</Dialog>

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
