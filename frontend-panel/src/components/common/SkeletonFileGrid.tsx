import { Skeleton } from "@/components/ui/skeleton";

interface SkeletonFileGridProps {
	count?: number;
}

export function SkeletonFileGrid({ count = 12 }: SkeletonFileGridProps) {
	return (
		<div className="p-4 space-y-4">
			<div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-3">
				{Array.from({ length: count }).map((_, i) => (
					<div
						// biome-ignore lint/suspicious/noArrayIndexKey: static skeleton placeholders never reorder
						key={`skeleton-card-${i}`}
						className="flex flex-col items-center p-3 rounded-lg border"
					>
						<Skeleton className="h-20 w-full mb-2 rounded-lg" />
						<Skeleton className="h-4 w-3/4 mb-1" />
						<Skeleton className="h-3 w-1/2" />
					</div>
				))}
			</div>
		</div>
	);
}
