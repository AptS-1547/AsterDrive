import { useTranslation } from "react-i18next";
import Markdown from "react-markdown";
import rehypeSanitize from "rehype-sanitize";
import remarkGfm from "remark-gfm";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useTextContent } from "@/hooks/useTextContent";

interface MarkdownPreviewProps {
	path: string;
}

export function MarkdownPreview({ path }: MarkdownPreviewProps) {
	const { t } = useTranslation("files");
	const { content, loading, error } = useTextContent(path);

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

	return (
		<div className="flex h-full min-h-0 w-full min-w-0 flex-col overflow-hidden rounded-xl border bg-background shadow-sm">
			<div className="border-b bg-muted/30 px-4 py-2 text-xs text-muted-foreground">
				Markdown · rendered
			</div>
			<div className="min-h-0 flex-1">
				<ScrollArea className="h-full rounded-b-xl bg-background">
					<div className="prose prose-sm dark:prose-invert max-w-none px-6 py-5">
						<Markdown
							remarkPlugins={[remarkGfm]}
							rehypePlugins={[rehypeSanitize]}
						>
							{content}
						</Markdown>
					</div>
				</ScrollArea>
			</div>
		</div>
	);
}
