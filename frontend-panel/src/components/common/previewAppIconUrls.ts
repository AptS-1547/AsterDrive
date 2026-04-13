const PREVIEW_APP_ICON_BASE = "/static/preview-apps";

export const PREVIEW_APP_ICON_URLS = {
	audio: `${PREVIEW_APP_ICON_BASE}/audio.svg`,
	code: `${PREVIEW_APP_ICON_BASE}/code.svg`,
	file: `${PREVIEW_APP_ICON_BASE}/file.svg`,
	googleDrive: `${PREVIEW_APP_ICON_BASE}/google-drive.svg`,
	image: `${PREVIEW_APP_ICON_BASE}/image.svg`,
	json: `${PREVIEW_APP_ICON_BASE}/json.svg`,
	markdown: `${PREVIEW_APP_ICON_BASE}/markdown.svg`,
	microsoftOnedrive: `${PREVIEW_APP_ICON_BASE}/microsoft-onedrive.svg`,
	pdf: `${PREVIEW_APP_ICON_BASE}/pdf.svg`,
	table: `${PREVIEW_APP_ICON_BASE}/table.svg`,
	video: `${PREVIEW_APP_ICON_BASE}/video.svg`,
	web: `${PREVIEW_APP_ICON_BASE}/web.svg`,
	xml: `${PREVIEW_APP_ICON_BASE}/xml.svg`,
} as const;

export function getBuiltinPreviewAppIconUrl(key: string) {
	switch (key.trim()) {
		case "builtin.audio":
			return PREVIEW_APP_ICON_URLS.audio;
		case "builtin.code":
			return PREVIEW_APP_ICON_URLS.code;
		case "builtin.try_text":
			return PREVIEW_APP_ICON_URLS.file;
		case "builtin.formatted":
			return PREVIEW_APP_ICON_URLS.json;
		case "builtin.image":
			return PREVIEW_APP_ICON_URLS.image;
		case "builtin.markdown":
			return PREVIEW_APP_ICON_URLS.markdown;
		case "builtin.office_google":
			return PREVIEW_APP_ICON_URLS.googleDrive;
		case "builtin.office_microsoft":
			return PREVIEW_APP_ICON_URLS.microsoftOnedrive;
		case "builtin.pdf":
			return PREVIEW_APP_ICON_URLS.pdf;
		case "builtin.table":
			return PREVIEW_APP_ICON_URLS.table;
		case "builtin.video":
			return PREVIEW_APP_ICON_URLS.video;
		default:
			return PREVIEW_APP_ICON_URLS.web;
	}
}
