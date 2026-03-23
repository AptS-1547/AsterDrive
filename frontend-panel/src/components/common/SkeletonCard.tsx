import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";

interface SkeletonCardProps {
	itemCount?: number;
}

export function SkeletonCard({ itemCount = 5 }: SkeletonCardProps) {
	return (
		<Card className="w-full max-w-lg">
			<CardHeader>
				<div className="flex items-center gap-2">
					<Skeleton className="h-5 w-5" />
					<Skeleton className="h-6 w-48" />
				</div>
				<Skeleton className="h-4 w-64 mt-2" />
			</CardHeader>
			<CardContent>
				<div className="space-y-1">
					{Array.from({ length: itemCount }).map((_, i) => (
						<div
							// biome-ignore lint/suspicious/noArrayIndexKey: static skeleton placeholders
							key={`skeleton-item-${i}`}
							className="flex items-center gap-2 px-3 py-2"
						>
							<Skeleton className="h-4 w-4 shrink-0" />
							<Skeleton
								className="h-4"
								style={{ width: `${50 + (i % 4) * 15}%` }}
							/>
						</div>
					))}
				</div>
			</CardContent>
		</Card>
	);
}
