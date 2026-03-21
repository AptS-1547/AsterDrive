import { Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { AdminLayout } from "@/components/layout/AdminLayout";
import {
	AlertDialog,
	AlertDialogAction,
	AlertDialogCancel,
	AlertDialogContent,
	AlertDialogDescription,
	AlertDialogFooter,
	AlertDialogHeader,
	AlertDialogTitle,
	AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
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
import { adminLockService } from "@/services/adminService";

interface WebdavLock {
	id: number;
	token: string;
	path: string;
	principal: string | null;
	owner_xml: string | null;
	timeout_at: string | null;
	shared: boolean;
	deep: boolean;
	created_at: string;
}

export default function AdminLocksPage() {
	const [locks, setLocks] = useState<WebdavLock[]>([]);
	const [loading, setLoading] = useState(true);

	const load = useCallback(async () => {
		try {
			setLoading(true);
			const data = await adminLockService.list();
			setLocks(data);
		} catch (e) {
			handleApiError(e);
		} finally {
			setLoading(false);
		}
	}, []);

	useEffect(() => {
		load();
	}, [load]);

	const handleForceUnlock = async (id: number) => {
		try {
			await adminLockService.forceUnlock(id);
			setLocks((prev) => prev.filter((l) => l.id !== id));
			toast.success("Lock released");
		} catch (e) {
			handleApiError(e);
		}
	};

	const handleCleanupExpired = async () => {
		try {
			const result = await adminLockService.cleanupExpired();
			toast.success(`Cleaned up ${result.removed} expired locks`);
			load();
		} catch (e) {
			handleApiError(e);
		}
	};

	const isExpired = (l: WebdavLock) =>
		l.timeout_at != null && new Date(l.timeout_at) < new Date();

	return (
		<AdminLayout>
			<div className="p-6 space-y-4">
				<div className="flex items-center justify-between">
					<h2 className="text-lg font-semibold">WebDAV Locks</h2>
					<Button variant="outline" size="sm" onClick={handleCleanupExpired}>
						Clean Expired
					</Button>
				</div>
				<ScrollArea className="flex-1">
					<Table>
						<TableHeader>
							<TableRow>
								<TableHead className="w-16">ID</TableHead>
								<TableHead>Path</TableHead>
								<TableHead>Principal</TableHead>
								<TableHead>Type</TableHead>
								<TableHead>Status</TableHead>
								<TableHead>Created</TableHead>
								<TableHead className="w-20">Actions</TableHead>
							</TableRow>
						</TableHeader>
						<TableBody>
							{loading ? (
								<TableRow>
									<TableCell
										colSpan={7}
										className="text-center text-muted-foreground"
									>
										Loading...
									</TableCell>
								</TableRow>
							) : locks.length === 0 ? (
								<TableRow>
									<TableCell
										colSpan={7}
										className="text-center text-muted-foreground"
									>
										No active locks
									</TableCell>
								</TableRow>
							) : (
								locks.map((l) => (
									<TableRow key={l.id}>
										<TableCell className="font-mono text-xs">{l.id}</TableCell>
										<TableCell className="font-mono text-xs max-w-[200px] truncate">
											{l.path}
										</TableCell>
										<TableCell className="text-xs">
											{l.principal ?? "-"}
										</TableCell>
										<TableCell>
											<div className="flex gap-1">
												<Badge variant="outline">
													{l.shared ? "Shared" : "Exclusive"}
												</Badge>
												{l.deep && <Badge variant="outline">Deep</Badge>}
											</div>
										</TableCell>
										<TableCell>
											{isExpired(l) ? (
												<Badge
													variant="outline"
													className="text-red-600 border-red-600"
												>
													Expired
												</Badge>
											) : (
												<Badge
													variant="outline"
													className="text-green-600 border-green-600"
												>
													Active
												</Badge>
											)}
										</TableCell>
										<TableCell className="text-muted-foreground text-xs">
											{new Date(l.created_at).toLocaleDateString()}
										</TableCell>
										<TableCell>
											<AlertDialog>
												<AlertDialogTrigger
													render={
														<Button
															variant="ghost"
															size="icon"
															className="h-8 w-8 text-destructive"
														/>
													}
												>
													<Trash2 className="h-3.5 w-3.5" />
												</AlertDialogTrigger>
												<AlertDialogContent>
													<AlertDialogHeader>
														<AlertDialogTitle>
															Force unlock "{l.path}"?
														</AlertDialogTitle>
														<AlertDialogDescription>
															This will release the WebDAV lock. Clients holding
															this lock will encounter errors.
														</AlertDialogDescription>
													</AlertDialogHeader>
													<AlertDialogFooter>
														<AlertDialogCancel>Cancel</AlertDialogCancel>
														<AlertDialogAction
															onClick={() => handleForceUnlock(l.id)}
														>
															Unlock
														</AlertDialogAction>
													</AlertDialogFooter>
												</AlertDialogContent>
											</AlertDialog>
										</TableCell>
									</TableRow>
								))
							)}
						</TableBody>
					</Table>
				</ScrollArea>
			</div>
		</AdminLayout>
	);
}
