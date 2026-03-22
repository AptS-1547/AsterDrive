import { ChevronLeft, ChevronRight } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { AdminLayout } from "@/components/layout/AdminLayout";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import { handleApiError } from "@/hooks/useApiError";
import type { AuditLogEntry } from "@/services/auditService";
import { auditService } from "@/services/auditService";

function formatDate(dateStr: string): string {
	return new Date(dateStr).toLocaleString();
}

export default function AdminAuditPage() {
	const [items, setItems] = useState<AuditLogEntry[]>([]);
	const [total, setTotal] = useState(0);
	const [offset, setOffset] = useState(0);
	const [actionFilter, setActionFilter] = useState("");
	const [entityTypeFilter, setEntityTypeFilter] = useState("");
	const [loading, setLoading] = useState(true);
	const limit = 20;

	const load = useCallback(async () => {
		setLoading(true);
		try {
			const page = await auditService.list({
				action: actionFilter || undefined,
				entity_type: entityTypeFilter || undefined,
				limit,
				offset,
			});
			setItems(page.items);
			setTotal(page.total);
		} catch (err) {
			handleApiError(err);
		} finally {
			setLoading(false);
		}
	}, [offset, actionFilter, entityTypeFilter]);

	useEffect(() => {
		load();
	}, [load]);

	const totalPages = Math.ceil(total / limit);
	const currentPage = Math.floor(offset / limit) + 1;

	return (
		<AdminLayout>
			<div className="p-4 space-y-4">
				<h2 className="text-lg font-semibold">Audit Log</h2>
				<div className="flex gap-2">
					<Input
						placeholder="Filter by action..."
						value={actionFilter}
						onChange={(e) => {
							setActionFilter(e.target.value);
							setOffset(0);
						}}
						className="max-w-xs"
					/>
					<select
						value={entityTypeFilter}
						onChange={(e) => {
							setEntityTypeFilter(e.target.value);
							setOffset(0);
						}}
						className="border rounded-md px-3 py-2 text-sm bg-background"
					>
						<option value="">All types</option>
						<option value="file">File</option>
						<option value="folder">Folder</option>
					</select>
				</div>

				<ScrollArea className="flex-1">
					<Table>
						<TableHeader>
							<TableRow>
								<TableHead>Time</TableHead>
								<TableHead>User</TableHead>
								<TableHead>Action</TableHead>
								<TableHead>Entity</TableHead>
								<TableHead>Name</TableHead>
								<TableHead>IP</TableHead>
							</TableRow>
						</TableHeader>
						<TableBody>
							{items.map((item) => (
								<TableRow key={item.id}>
									<TableCell className="text-xs text-muted-foreground whitespace-nowrap">
										{formatDate(item.created_at)}
									</TableCell>
									<TableCell>{item.user_id}</TableCell>
									<TableCell>
										<span className="inline-flex items-center rounded-full bg-blue-50 px-2 py-0.5 text-xs font-medium text-blue-700">
											{item.action}
										</span>
									</TableCell>
									<TableCell className="text-muted-foreground">
										{item.entity_type ?? "---"}
									</TableCell>
									<TableCell>{item.entity_name ?? "---"}</TableCell>
									<TableCell className="text-xs text-muted-foreground">
										{item.ip_address ?? "---"}
									</TableCell>
								</TableRow>
							))}
							{!loading && items.length === 0 && (
								<TableRow>
									<TableCell
										colSpan={6}
										className="text-center py-8 text-muted-foreground"
									>
										No audit log entries
									</TableCell>
								</TableRow>
							)}
						</TableBody>
					</Table>
				</ScrollArea>

				{totalPages > 1 && (
					<div className="flex items-center justify-between">
						<span className="text-sm text-muted-foreground">
							{total} entries, page {currentPage} of {totalPages}
						</span>
						<div className="flex gap-1">
							<Button
								variant="outline"
								size="sm"
								disabled={offset === 0}
								onClick={() => setOffset(Math.max(0, offset - limit))}
							>
								<ChevronLeft className="h-4 w-4" />
							</Button>
							<Button
								variant="outline"
								size="sm"
								disabled={offset + limit >= total}
								onClick={() => setOffset(offset + limit)}
							>
								<ChevronRight className="h-4 w-4" />
							</Button>
						</div>
					</div>
				)}
			</div>
		</AdminLayout>
	);
}
