import { Skeleton } from "@/components/ui/skeleton";
import { PAGE_SECTION_PADDING_CLASS } from "@/lib/constants";
import { SharePageShell } from "./ShareViewShell";

const SHARE_GRID_SKELETON_ITEMS = [
	{ key: "alpha", width: "72%" },
	{ key: "bravo", width: "54%" },
	{ key: "charlie", width: "64%" },
	{ key: "delta", width: "48%" },
	{ key: "echo", width: "68%" },
	{ key: "foxtrot", width: "52%" },
	{ key: "golf", width: "58%" },
	{ key: "hotel", width: "46%" },
	{ key: "india", width: "62%" },
	{ key: "juliet", width: "50%" },
	{ key: "kilo", width: "66%" },
	{ key: "lima", width: "56%" },
];
const SHARE_TABLE_SKELETON_WIDTHS = [
	"68%",
	"52%",
	"60%",
	"48%",
	"64%",
	"56%",
	"50%",
	"62%",
];

export function ShareLoadingSkeleton() {
	return (
		<SharePageShell>
			<main className="flex min-h-0 flex-1 flex-col overflow-hidden">
				<div
					className={`border-b border-border/65 bg-card/55 ${PAGE_SECTION_PADDING_CLASS}`}
				>
					<div className="mx-auto flex w-full max-w-7xl items-center gap-3 py-3">
						<Skeleton className="size-12 rounded-lg" />
						<div className="min-w-0 flex-1 space-y-2">
							<Skeleton className="h-5 w-3/5 max-w-lg" />
							<Skeleton className="h-3 w-72 max-w-full" />
						</div>
						<Skeleton className="hidden h-8 w-20 rounded-lg sm:block" />
					</div>
				</div>
				<div className={`min-h-0 flex-1 py-3 ${PAGE_SECTION_PADDING_CLASS}`}>
					<section className="mx-auto flex h-full w-full max-w-7xl flex-col overflow-hidden rounded-lg border border-border/70 bg-card/65 shadow-xs dark:bg-card/35 dark:shadow-none">
						<ShareFolderContentSkeleton viewMode="grid" />
					</section>
				</div>
			</main>
		</SharePageShell>
	);
}

export function ShareFolderContentSkeleton({
	viewMode,
}: {
	viewMode: "grid" | "list";
}) {
	if (viewMode === "list") {
		return (
			<div className="divide-y divide-border/60">
				<div className="grid grid-cols-[minmax(0,1fr)_6rem_8rem_2rem] gap-3 px-4 py-3">
					<Skeleton className="h-4 w-24" />
					<Skeleton className="h-4 w-14" />
					<Skeleton className="h-4 w-20" />
					<Skeleton className="size-4" />
				</div>
				{SHARE_TABLE_SKELETON_WIDTHS.map((width) => (
					<div
						key={`share-row-${width}`}
						className="grid grid-cols-[minmax(0,1fr)_6rem_8rem_2rem] items-center gap-3 px-4 py-3"
					>
						<div className="flex min-w-0 items-center gap-2">
							<Skeleton className="size-8 rounded-lg" />
							<Skeleton className="h-4" style={{ width }} />
						</div>
						<Skeleton className="h-4 w-14" />
						<Skeleton className="h-4 w-20" />
						<Skeleton className="size-7 rounded-lg" />
					</div>
				))}
			</div>
		);
	}

	return (
		<div className="grid grid-cols-2 gap-3 p-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6">
			{SHARE_GRID_SKELETON_ITEMS.map((item) => (
				<div
					key={`share-grid-${item.key}`}
					className="rounded-lg border border-border/65 bg-card/70 p-3 shadow-xs dark:bg-card/40 dark:shadow-none"
				>
					<Skeleton className="mb-3 aspect-[4/3] w-full rounded-lg" />
					<Skeleton className="mb-2 h-4" style={{ width: item.width }} />
					<Skeleton className="h-3 w-1/2" />
				</div>
			))}
		</div>
	);
}
