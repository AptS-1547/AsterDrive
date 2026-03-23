import { Highlight, themes } from "prism-react-renderer";
import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useTextContent } from "@/hooks/useTextContent";

interface JsonPreviewProps {
	path: string;
}

export function JsonPreview({ path }: JsonPreviewProps) {
	const { t } = useTranslation("files");
	const { content, loading, error } = useTextContent(path);

	const formatted = useMemo(() => {
		if (!content) return null;
		try {
			return JSON.stringify(JSON.parse(content), null, 2);
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
				JSON · formatted
			</div>
			<div className="min-h-0 flex-1">
				<ScrollArea className="h-full bg-background">
					<Highlight theme={themes.github} code={formatted} language="json">
						{({ className, style, tokens, getLineProps, getTokenProps }) => (
							<pre
								className={`${className} min-h-full p-4 font-mono text-sm leading-6 whitespace-pre-wrap break-words`}
								style={{ ...style, background: "transparent", margin: 0 }}
							>
								{tokens.map((line) => {
									const lineText = line.map((token) => token.content).join("");
									const lineKey = `line-${lineText}`;
									const lineProps = getLineProps({ line, key: lineKey });
									return (
										<div key={lineKey} {...lineProps}>
											{line.map((token) => {
												const tokenKey = `${lineKey}-${token.types.join("-")}-${token.content}`;
												const tokenProps = getTokenProps({
													key: tokenKey,
													token,
												});
												return <span key={tokenKey} {...tokenProps} />;
											})}
										</div>
									);
								})}
							</pre>
						)}
					</Highlight>
				</ScrollArea>
			</div>
		</div>
	);
}
