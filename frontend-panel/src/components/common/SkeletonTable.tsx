import { Skeleton } from "@/components/ui/skeleton";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";

interface SkeletonTableProps {
	columns?: number;
	rows?: number;
}

export function SkeletonTable({ columns = 4, rows = 8 }: SkeletonTableProps) {
	return (
		<Table>
			<TableHeader>
				<TableRow>
					{Array.from({ length: columns }).map((_, i) => (
						// biome-ignore lint/suspicious/noArrayIndexKey: static skeleton placeholders never reorder
						<TableHead key={`skeleton-head-${i}`}>
							<Skeleton
								className="h-4"
								style={{ width: `${50 + (i % 4) * 15}%` }}
							/>
						</TableHead>
					))}
				</TableRow>
			</TableHeader>
			<TableBody>
				{Array.from({ length: rows }).map((_, rowIdx) => (
					// biome-ignore lint/suspicious/noArrayIndexKey: static skeleton placeholders never reorder
					<TableRow key={`skeleton-row-${rowIdx}`}>
						{Array.from({ length: columns }).map((_, colIdx) => (
							// biome-ignore lint/suspicious/noArrayIndexKey: static skeleton placeholders never reorder
							<TableCell key={`skeleton-cell-${colIdx}`}>
								<Skeleton
									className="h-4"
									style={{
										width: `${60 + ((rowIdx + colIdx) % 4) * 10}%`,
									}}
								/>
							</TableCell>
						))}
					</TableRow>
				))}
			</TableBody>
		</Table>
	);
}
