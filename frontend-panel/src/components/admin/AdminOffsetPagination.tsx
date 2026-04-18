import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import {
	Tooltip,
	TooltipContent,
	TooltipProvider,
	TooltipTrigger,
} from "@/components/ui/tooltip";

interface AdminOffsetPaginationProps {
	currentPage: number;
	nextDisabled: boolean;
	onNext: () => void;
	onPageSizeChange: (value: string | null) => void;
	onPrevious: () => void;
	pageSize: string;
	pageSizeOptions: Array<{ label: string; value: string }>;
	prevDisabled: boolean;
	total: number;
	totalPages: number;
}

export function AdminOffsetPagination({
	currentPage,
	nextDisabled,
	onNext,
	onPageSizeChange,
	onPrevious,
	pageSize,
	pageSizeOptions,
	prevDisabled,
	total,
	totalPages,
}: AdminOffsetPaginationProps) {
	const { t } = useTranslation("admin");

	if (total <= 0) {
		return null;
	}

	return (
		<div className="flex items-center justify-between gap-3 px-4 pb-4 text-sm text-muted-foreground md:px-6">
			<div className="flex items-center gap-3">
				<span>
					{t("entries_page", {
						total,
						current: currentPage,
						pages: totalPages,
					})}
				</span>
				<Select
					items={pageSizeOptions}
					value={pageSize}
					onValueChange={onPageSizeChange}
				>
					<SelectTrigger width="page-size">
						<SelectValue />
					</SelectTrigger>
					<SelectContent>
						{pageSizeOptions.map((option) => (
							<SelectItem key={option.value} value={option.value}>
								{option.label}
							</SelectItem>
						))}
					</SelectContent>
				</Select>
			</div>
			<TooltipProvider>
				<div className="flex items-center gap-2">
					<Tooltip>
						<TooltipTrigger
							render={
								<Button
									variant="outline"
									size="sm"
									disabled={prevDisabled}
									onClick={onPrevious}
								/>
							}
						>
							<Icon name="CaretLeft" className="h-4 w-4" />
						</TooltipTrigger>
						{prevDisabled ? (
							<TooltipContent>{t("pagination_prev_disabled")}</TooltipContent>
						) : null}
					</Tooltip>
					<Tooltip>
						<TooltipTrigger
							render={
								<Button
									variant="outline"
									size="sm"
									disabled={nextDisabled}
									onClick={onNext}
								/>
							}
						>
							<Icon name="CaretRight" className="h-4 w-4" />
						</TooltipTrigger>
						{nextDisabled ? (
							<TooltipContent>{t("pagination_next_disabled")}</TooltipContent>
						) : null}
					</Tooltip>
				</div>
			</TooltipProvider>
		</div>
	);
}
