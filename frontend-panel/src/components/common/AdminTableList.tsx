import type { ReactNode } from "react";
import { EmptyState } from "@/components/common/EmptyState";
import { SkeletonTable } from "@/components/common/SkeletonTable";
import { AdminSurface } from "@/components/layout/AdminSurface";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Table, TableBody } from "@/components/ui/table";

interface AdminTableListProps<T> {
	loading: boolean;
	items: T[];
	columns: number;
	rows?: number;
	emptyIcon?: ReactNode;
	emptyTitle: string;
	emptyDescription?: string;
	headerRow: ReactNode;
	renderRow: (item: T) => ReactNode;
}

export function AdminTableList<T>({
	loading,
	items,
	columns,
	rows,
	emptyIcon,
	emptyTitle,
	emptyDescription,
	headerRow,
	renderRow,
}: AdminTableListProps<T>) {
	if (loading) {
		return <SkeletonTable columns={columns} rows={rows ?? 5} />;
	}

	if (items.length === 0) {
		return (
			<EmptyState
				icon={emptyIcon}
				title={emptyTitle}
				description={emptyDescription}
			/>
		);
	}

	return (
		<AdminSurface>
			<ScrollArea className="min-h-0 flex-1">
				<Table>
					{headerRow}
					<TableBody>{items.map(renderRow)}</TableBody>
				</Table>
			</ScrollArea>
		</AdminSurface>
	);
}
