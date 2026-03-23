import { Skeleton } from "@/components/ui/skeleton";

interface SkeletonTreeProps {
	count?: number;
}

export function SkeletonTree({ count = 5 }: SkeletonTreeProps) {
	return (
		<div className="p-2 space-y-0.5">
			{Array.from({ length: count }).map((_, i) => (
				<div
					// biome-ignore lint/suspicious/noArrayIndexKey: static skeleton placeholders never reorder
					key={`skeleton-node-${i}`}
					className="flex items-center gap-2 px-2 py-1.5"
					style={{ paddingLeft: `${(i % 3) * 16 + 8}px` }}
				>
					<Skeleton className="h-3 w-3 shrink-0" />
					<Skeleton className="h-4 w-4 shrink-0" />
					<Skeleton
						className="h-4"
						style={{ width: `${50 + (i % 4) * 15}%` }}
					/>
				</div>
			))}
		</div>
	);
}
