import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import xmlFormatter from "xml-formatter";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useTextContent } from "@/hooks/useTextContent";

interface XmlPreviewProps {
	path: string;
	mode: "formatted";
}

export function XmlPreview({ path }: XmlPreviewProps) {
	const { t } = useTranslation("files");
	const { content, loading, error } = useTextContent(path);

	const formatted = useMemo(() => {
		if (!content) return null;
		const doc = new DOMParser().parseFromString(content, "application/xml");
		if (doc.querySelector("parsererror")) return null;
		try {
			return xmlFormatter(content, {
				indentation: "  ",
				lineSeparator: "\n",
				collapseContent: false,
			});
		} catch {
			return null;
		}
	}, [content]);

	if (loading) {
		return (
			<div className="p-6 text-sm text-muted-foreground">
				{t("loading_preview")}
			</div>
		);
	}

	if (error || content === null) {
		return (
			<div className="p-6 text-sm text-destructive">
				{t("preview_load_failed")}
			</div>
		);
	}

	if (!formatted) {
		return (
			<div className="p-6 text-sm text-destructive">
				{t("structured_parse_failed")}
			</div>
		);
	}

	return (
		<div className="flex h-full min-h-0 w-full min-w-0 flex-col overflow-hidden rounded-xl border bg-background shadow-sm">
			<div className="border-b bg-muted/30 px-4 py-2 text-xs text-muted-foreground">
				XML · formatted
			</div>
			<div className="min-h-0 flex-1">
				<ScrollArea className="h-full bg-background">
					<pre className="min-h-full p-4 font-mono text-sm whitespace-pre-wrap break-words">
						{formatted}
					</pre>
				</ScrollArea>
			</div>
		</div>
	);
}
