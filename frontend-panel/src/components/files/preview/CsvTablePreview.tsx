import Papa from "papaparse";
import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import { useTextContent } from "@/hooks/useTextContent";
import { PreviewError } from "./PreviewError";

interface CsvTablePreviewProps {
	path: string;
	delimiter: "," | "\t";
}

const MAX_ROWS = 500;

export function CsvTablePreview({ path, delimiter }: CsvTablePreviewProps) {
	const { t } = useTranslation(["files", "common"]);
	const { content, loading, error, reload } = useTextContent(path);

	const parsed = useMemo(() => {
		if (!content) return null;
		return Papa.parse<string[]>(content, {
			delimiter,
			skipEmptyLines: true,
		});
	}, [content, delimiter]);

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

	if (!parsed || parsed.errors.length > 0 || parsed.data.length === 0) {
		return (
			<div className="p-6 text-sm text-destructive">
				{t("files:table_parse_failed")}
			</div>
		);
	}

	const rows = parsed.data.slice(0, MAX_ROWS);
	const header = rows[0] ?? [];
	const body = rows.slice(1);
	const headerKey = header.join("|");

	return (
		<div className="overflow-hidden rounded-xl border bg-background">
			<Table>
				<TableHeader>
					<TableRow>
						{header.map((cell, index) => (
							<TableHead
								key={`header-${headerKey}-${cell || `column-${index + 1}`}`}
								className="whitespace-pre-wrap break-words"
							>
								{cell || `${t("common:column")} ${index + 1}`}
							</TableHead>
						))}
					</TableRow>
				</TableHeader>
				<TableBody>
					{body.map((row) => {
						const rowKey = row.join("|");
						return (
							<TableRow key={`row-${rowKey}`}>
								{header.map((_, cellIndex) => (
									<TableCell
										key={`cell-${rowKey}-${header[cellIndex] ?? `column-${cellIndex + 1}`}`}
										className="max-w-80 whitespace-pre-wrap break-words align-top"
									>
										{row[cellIndex] ?? ""}
									</TableCell>
								))}
							</TableRow>
						);
					})}
				</TableBody>
			</Table>
			{parsed.data.length > MAX_ROWS && (
				<div className="border-t bg-muted/40 px-4 py-2 text-xs text-muted-foreground">
					{t("files:table_truncated", { count: MAX_ROWS })}
				</div>
			)}
		</div>
	);
}
