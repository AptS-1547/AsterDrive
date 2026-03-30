export interface WarmupLoaderEntry {
	key: string;
	label: string;
	load: () => Promise<unknown>;
}

const loginRouteWarmupLoader = {
	key: "route:login",
	label: "LoginPage",
	load: () => import("@/pages/LoginPage"),
} satisfies WarmupLoaderEntry;

export const userRouteWarmupLoaders = [
	loginRouteWarmupLoader,
	{
		key: "route:my-shares",
		label: "MySharesPage",
		load: () => import("@/pages/MySharesPage"),
	},
	{
		key: "route:trash",
		label: "TrashPage",
		load: () => import("@/pages/TrashPage"),
	},
	{
		key: "route:settings",
		label: "SettingsPage",
		load: () => import("@/pages/SettingsPage"),
	},
	{
		key: "route:webdav-accounts",
		label: "WebdavAccountsPage",
		load: () => import("@/pages/WebdavAccountsPage"),
	},
] satisfies WarmupLoaderEntry[];

export const adminRouteWarmupLoaders = [
	{
		key: "route:admin-users",
		label: "AdminUsersPage",
		load: () => import("@/pages/admin/AdminUsersPage"),
	},
	{
		key: "route:admin-policies",
		label: "AdminPoliciesPage",
		load: () => import("@/pages/admin/AdminPoliciesPage"),
	},
	{
		key: "route:admin-settings",
		label: "AdminSettingsPage",
		load: () => import("@/pages/admin/AdminSettingsPage"),
	},
	{
		key: "route:admin-shares",
		label: "AdminSharesPage",
		load: () => import("@/pages/admin/AdminSharesPage"),
	},
	{
		key: "route:admin-locks",
		label: "AdminLocksPage",
		load: () => import("@/pages/admin/AdminLocksPage"),
	},
	{
		key: "route:admin-audit",
		label: "AdminAuditPage",
		load: () => import("@/pages/admin/AdminAuditPage"),
	},
	{
		key: "route:admin-about",
		label: "AdminAboutPage",
		load: () => import("@/pages/admin/AdminAboutPage"),
	},
] satisfies WarmupLoaderEntry[];

export const userFeatureWarmupLoaders = [
	{
		key: "feature:file-preview",
		label: "FilePreview",
		load: () => import("@/components/files/FilePreview"),
	},
	{
		key: "feature:language-icons",
		label: "LanguageIcons",
		load: () =>
			import("@/components/ui/language-icon").then((module) =>
				module.loadLanguageIcons(),
			),
	},
	{
		key: "feature:upload-area",
		label: "UploadArea",
		load: () => import("@/components/files/UploadArea"),
	},
	{
		key: "feature:share-dialog",
		label: "ShareDialog",
		load: () => import("@/components/files/ShareDialog"),
	},
	{
		key: "feature:file-info-dialog",
		label: "FileInfoDialog",
		load: () => import("@/components/files/FileInfoDialog"),
	},
	{
		key: "feature:rename-dialog",
		label: "RenameDialog",
		load: () => import("@/components/files/RenameDialog"),
	},
	{
		key: "feature:version-history-dialog",
		label: "VersionHistoryDialog",
		load: () => import("@/components/files/VersionHistoryDialog"),
	},
	{
		key: "feature:batch-target-folder-dialog",
		label: "BatchTargetFolderDialog",
		load: () => import("@/components/files/BatchTargetFolderDialog"),
	},
	{
		key: "feature:create-file-dialog",
		label: "CreateFileDialog",
		load: () => import("@/components/files/CreateFileDialog"),
	},
	{
		key: "feature:create-folder-dialog",
		label: "CreateFolderDialog",
		load: () => import("@/components/files/CreateFolderDialog"),
	},
] satisfies WarmupLoaderEntry[];

export const filePreviewWarmupLoaders = [
	{
		key: "preview:text-code",
		label: "TextCodePreview",
		load: () => import("@/components/files/preview/TextCodePreview"),
	},
	{
		key: "preview:json",
		label: "JsonPreview",
		load: () => import("@/components/files/preview/JsonPreview"),
	},
	{
		key: "preview:xml",
		label: "XmlPreview",
		load: () => import("@/components/files/preview/XmlPreview"),
	},
	{
		key: "preview:csv",
		label: "CsvTablePreview",
		load: () => import("@/components/files/preview/CsvTablePreview"),
	},
	{
		key: "preview:markdown",
		label: "MarkdownPreview",
		load: () => import("@/components/files/preview/MarkdownPreview"),
	},
	{
		key: "preview:pdf",
		label: "PdfPreview",
		load: () => import("@/components/files/preview/PdfPreview"),
	},
] satisfies WarmupLoaderEntry[];
