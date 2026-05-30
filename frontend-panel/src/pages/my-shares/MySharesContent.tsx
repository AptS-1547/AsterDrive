import { EmptyState } from "@/components/common/EmptyState";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Icon } from "@/components/ui/icon";
import type { MyShareInfo } from "@/types/api";
import { MyShareCard } from "./MyShareCard";

interface MySharesContentLabels {
	active: string;
	copy: string;
	created: (date: string) => string;
	delete: string;
	deleted: string;
	edit: string;
	emptyDescription: string;
	emptyTitle: string;
	exhausted: string;
	expire: (date: string) => string;
	expired: string;
	never: string;
	next: string;
	open: string;
	pageDescription: string;
	prev: string;
}

interface MySharesContentProps {
	labels: MySharesContentLabels;
	loading: boolean;
	onCopy: (share: MyShareInfo) => void;
	onDelete: (share: MyShareInfo) => void;
	onEdit: (share: MyShareInfo) => void;
	onNextPage: () => void;
	onOpen: (share: MyShareInfo) => void;
	onPrevPage: () => void;
	onToggleSelect: (shareId: number) => void;
	page: number;
	selectedShareIds: Set<number>;
	shares: MyShareInfo[];
	totalPages: number;
}

export function MySharesContent({
	labels,
	loading,
	onCopy,
	onDelete,
	onEdit,
	onNextPage,
	onOpen,
	onPrevPage,
	onToggleSelect,
	page,
	selectedShareIds,
	shares,
	totalPages,
}: MySharesContentProps) {
	if (loading) {
		return (
			<div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
				{["s1", "s2", "s3", "s4", "s5", "s6"].map((key) => (
					<Card key={key} className="h-28 animate-pulse bg-muted/20" />
				))}
			</div>
		);
	}

	if (shares.length === 0) {
		return (
			<Card className="bg-muted/15">
				<div className="py-12">
					<EmptyState
						icon={<Icon name="Link" className="size-10" />}
						title={labels.emptyTitle}
						description={labels.emptyDescription}
					/>
				</div>
			</Card>
		);
	}

	return (
		<>
			<div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
				{shares.map((share) => (
					<MyShareCard
						key={share.id}
						share={share}
						selected={selectedShareIds.has(share.id)}
						labels={labels}
						onCopy={onCopy}
						onDelete={onDelete}
						onEdit={onEdit}
						onOpen={onOpen}
						onToggleSelect={onToggleSelect}
					/>
				))}
			</div>

			<div className="flex items-center justify-between rounded-xl border bg-muted/15 px-4 py-3">
				<p className="text-sm text-muted-foreground">
					{labels.pageDescription}
				</p>
				<div className="flex items-center gap-2">
					<Button
						variant="outline"
						size="sm"
						disabled={page === 0}
						onClick={onPrevPage}
					>
						{labels.prev}
					</Button>
					<Button
						variant="outline"
						size="sm"
						disabled={page + 1 >= totalPages}
						onClick={onNextPage}
					>
						{labels.next}
					</Button>
				</div>
			</div>
		</>
	);
}
