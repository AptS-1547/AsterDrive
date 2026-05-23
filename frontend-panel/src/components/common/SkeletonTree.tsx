import { Skeleton } from "@/components/ui/skeleton";
import {
	FOLDER_TREE_INDENT_PX,
	FOLDER_TREE_SKELETON_OFFSET_PX,
} from "@/lib/constants";

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
					className="flex items-center gap-1 rounded-md px-2 py-1.5"
					style={{
						paddingLeft: `${(i % 3) * FOLDER_TREE_INDENT_PX + FOLDER_TREE_SKELETON_OFFSET_PX}px`,
					}}
				>
					<Skeleton className="size-3 shrink-0" />
					<Skeleton className="size-4 shrink-0" />
					<Skeleton
						className="h-4"
						style={{ width: `${50 + (i % 4) * 15}%` }}
					/>
				</div>
			))}
		</div>
	);
}
