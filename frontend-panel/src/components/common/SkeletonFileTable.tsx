import { Skeleton } from "@/components/ui/skeleton";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";

interface SkeletonFileTableProps {
	rows?: number;
}

export function SkeletonFileTable({ rows = 8 }: SkeletonFileTableProps) {
	return (
		<Table>
			<TableHeader>
				<TableRow>
					<TableHead className="w-10">
						<Skeleton className="h-4 w-4" />
					</TableHead>
					<TableHead>
						<Skeleton className="h-4 w-24" />
					</TableHead>
					<TableHead>
						<Skeleton className="h-4 w-20" />
					</TableHead>
				</TableRow>
			</TableHeader>
			<TableBody>
				{Array.from({ length: rows }).map((_, i) => (
					// biome-ignore lint/suspicious/noArrayIndexKey: static skeleton placeholders never reorder
					<TableRow key={`skeleton-row-${i}`}>
						<TableCell>
							<Skeleton className="h-4 w-4" />
						</TableCell>
						<TableCell>
							<div className="flex items-center gap-2">
								<Skeleton className="h-4 w-4 shrink-0" />
								<Skeleton
									className="h-4"
									style={{ width: `${60 + (i % 3) * 10}%` }}
								/>
							</div>
						</TableCell>
						<TableCell>
							<Skeleton className="h-4 w-32" />
						</TableCell>
					</TableRow>
				))}
			</TableBody>
		</Table>
	);
}
