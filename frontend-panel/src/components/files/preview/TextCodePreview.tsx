import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import { useFileEditorSession } from "@/hooks/useFileEditorSession";
import { useTextContent } from "@/hooks/useTextContent";
import { getEditorLanguage } from "./file-capabilities";
import {
	MonacoCodeEditor,
	type MonacoCodeEditorMountHandler,
} from "./MonacoCodeEditor";
import { PreviewError } from "./PreviewError";
import type { PreviewableFileLike } from "./types";

interface TextCodePreviewProps {
	file: PreviewableFileLike & { id: number };
	path: string;
	onFileUpdated?: () => void;
	onDirtyChange?: (dirty: boolean) => void;
	editable?: boolean;
}

function useIsDark() {
	const [dark, setDark] = useState(
		document.documentElement.classList.contains("dark"),
	);

	useEffect(() => {
		const observer = new MutationObserver(() => {
			setDark(document.documentElement.classList.contains("dark"));
		});
		observer.observe(document.documentElement, {
			attributes: true,
			attributeFilter: ["class"],
		});
		return () => observer.disconnect();
	}, []);

	return dark;
}

export function TextCodePreview({
	file,
	path,
	onFileUpdated,
	onDirtyChange,
	editable = true,
}: TextCodePreviewProps) {
	const { t } = useTranslation(["core", "files"]);
	const isDark = useIsDark();
	const { content, etag, loading, error, reload } = useTextContent(path);
	const {
		editing,
		dirty,
		editContent,
		saving,
		setEditContent,
		startEditing,
		cancelEditing,
		save,
	} = useFileEditorSession({
		fileId: file.id,
		initialContent: content ?? "",
		etag,
		onSaved: async () => {
			await reload();
			await onFileUpdated?.();
		},
		onConflict: () => reload(),
		messages: {
			saved: t("files:file_saved"),
			editedByOthers: t("files:edited_by_others"),
		},
	});
	const saveRef = useRef(save);

	useEffect(() => {
		saveRef.current = save;
	}, [save]);

	useEffect(() => {
		onDirtyChange?.(dirty);
	}, [dirty, onDirtyChange]);

	const handleEditorMount = useCallback<MonacoCodeEditorMountHandler>(
		(editor, monaco) => {
			editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => {
				saveRef.current();
			});
		},
		[],
	);

	if (loading) {
		return (
			<div className="p-6 text-sm text-muted-foreground">
				{t("files:loading_preview")}
			</div>
		);
	}

	if (error || content === null) {
		return <PreviewError onRetry={() => void reload()} />;
	}

	const language = getEditorLanguage(file);

	return (
		<div className="flex h-full min-h-0 w-full min-w-0 flex-col overflow-hidden rounded-xl border bg-background shadow-sm">
			<div className="flex items-center gap-2 border-b bg-muted/40 px-4 py-2">
				<div className="flex items-center gap-2">
					<Icon name="FileCode" className="h-4 w-4 text-muted-foreground" />
					<span className="text-sm font-medium">{file.name}</span>
				</div>
				<div className="ml-auto flex items-center gap-2">
					{!editing ? (
						editable ? (
							<Button variant="outline" size="sm" onClick={startEditing}>
								<Icon name="PencilSimple" className="mr-1 h-3.5 w-3.5" />
								{t("edit")}
							</Button>
						) : null
					) : (
						<>
							<Button
								variant="default"
								size="sm"
								onClick={save}
								disabled={saving}
							>
								<Icon name="FloppyDisk" className="mr-1 h-3.5 w-3.5" />
								{saving ? t("files:saving") : t("save")}
							</Button>
							<Button variant="outline" size="sm" onClick={cancelEditing}>
								<Icon name="Undo" className="mr-1 h-3.5 w-3.5" />
								{t("cancel")}
							</Button>
						</>
					)}
				</div>
			</div>
			<div className="flex items-center gap-3 border-b bg-background px-4 py-2 text-xs text-muted-foreground">
				<span>{language}</span>
				<span>·</span>
				<span>{editable && editing ? t("edit") : t("open_with_code")}</span>
				<span>·</span>
				<span>{dirty ? t("files:unsaved_changes") : t("active")}</span>
				{editing ? (
					<>
						<span>·</span>
						<span>{t("files:save_shortcut_hint")}</span>
					</>
				) : null}
			</div>
			<div className="min-h-0 w-full min-w-0 flex-1 overflow-hidden bg-background">
				<MonacoCodeEditor
					key={path}
					language={language}
					theme={isDark ? "vs-dark" : "vs"}
					value={editing ? editContent : content}
					onChange={(value) => setEditContent(value ?? "")}
					onMount={handleEditorMount}
					options={{
						readOnly: !editing,
						minimap: { enabled: true },
						wordWrap: "on",
						fontSize: 13,
						lineNumbers: "on",
						scrollBeyondLastLine: false,
						renderLineHighlight: editing ? "line" : "none",
						domReadOnly: !editing,
						padding: { top: 12 },
					}}
				/>
			</div>
		</div>
	);
}
