import type { FormEvent } from "react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { useSearchParams } from "react-router-dom";
import { toast } from "sonner";
import { z } from "zod/v4";
import { UserDetailDialog } from "@/components/admin/UserDetailDialog";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
import { EmptyState } from "@/components/common/EmptyState";
import { SkeletonTable } from "@/components/common/SkeletonTable";
import { AdminLayout } from "@/components/layout/AdminLayout";
import { AdminPageHeader } from "@/components/layout/AdminPageHeader";
import { AdminPageShell } from "@/components/layout/AdminPageShell";
import { AdminSurface } from "@/components/layout/AdminSurface";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Icon } from "@/components/ui/icon";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import {
	Table,
	TableBody,
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import {
	Tooltip,
	TooltipContent,
	TooltipProvider,
	TooltipTrigger,
} from "@/components/ui/tooltip";
import { handleApiError } from "@/hooks/useApiError";
import {
	ADMIN_CONTROL_HEIGHT_CLASS,
	ADMIN_ICON_BUTTON_CLASS,
} from "@/lib/constants";
import { formatBytes } from "@/lib/format";
import { adminUserService } from "@/services/adminService";
import type {
	CreateUserReq,
	UserInfo,
	UserRole,
	UserStatus,
} from "@/types/api";

const USER_PAGE_SIZE_OPTIONS = [10, 20, 50] as const;

const createUserUsernameSchema = z
	.string()
	.min(4, "Username must be 4-16 characters")
	.max(16, "Username must be 4-16 characters")
	.regex(/^[a-zA-Z0-9_-]+$/, "Only letters, numbers, _ and -");
const createUserEmailSchema = z
	.string()
	.max(254, "Email is too long")
	.regex(/^[^@]+@[^@]+\.[^@]+$/, "Invalid email format");
const createUserPasswordSchema = z
	.string()
	.min(6, "Password must be at least 6 characters")
	.max(128, "Password must be at most 128 characters");

function getRoleBadgeClass(role: UserRole) {
	return role === "admin"
		? "border-blue-500/60 bg-blue-500/10 text-blue-600 dark:text-blue-300"
		: "border-border bg-muted/40 text-muted-foreground";
}

function getStatusBadgeClass(status: UserStatus) {
	return status === "active"
		? "border-green-500/60 bg-green-500/10 text-green-600 dark:text-green-300"
		: "border-amber-500/60 bg-amber-500/10 text-amber-600 dark:text-amber-300";
}

function QuotaCell({ user }: { user: UserInfo }) {
	const { t } = useTranslation("admin");
	const quota = user.storage_quota ?? 0;
	const used = user.storage_used ?? 0;
	const pct = quota > 0 ? Math.min((used / quota) * 100, 100) : 0;

	return (
		<div className="flex w-full flex-col gap-2 rounded-lg border border-transparent bg-muted/20 px-3 py-2 text-left">
			<div className="flex items-center justify-between gap-3 text-xs">
				<span className="font-medium text-foreground">
					{formatBytes(used)}
					{quota > 0
						? ` / ${formatBytes(quota)}`
						: ` / ${t("common:unlimited")}`}
				</span>
				<span className="text-[11px] text-muted-foreground">
					{quota > 0 ? `${Math.round(pct)}%` : t("quota_unlimited_short")}
				</span>
			</div>
			{quota > 0 ? <Progress value={pct} className="h-1.5" /> : null}
		</div>
	);
}

export default function AdminUsersPage() {
	const { t } = useTranslation("admin");
	const [searchParams, setSearchParams] = useSearchParams();
	const initialKeyword = searchParams.get("keyword") ?? "";
	const initialRole = searchParams.get("role");
	const initialStatus = searchParams.get("status");
	const initialOffset = Number(searchParams.get("offset") ?? "0");
	const initialPageSize = Number(searchParams.get("pageSize") ?? "20");
	const [users, setUsers] = useState<UserInfo[]>([]);
	const [loading, setLoading] = useState(true);
	const [total, setTotal] = useState(0);
	const [offset, setOffset] = useState(
		Number.isNaN(initialOffset) ? 0 : initialOffset,
	);
	const [pageSize, setPageSize] = useState<
		(typeof USER_PAGE_SIZE_OPTIONS)[number]
	>(
		USER_PAGE_SIZE_OPTIONS.includes(
			initialPageSize as (typeof USER_PAGE_SIZE_OPTIONS)[number],
		)
			? (initialPageSize as (typeof USER_PAGE_SIZE_OPTIONS)[number])
			: 20,
	);
	const [keyword, setKeyword] = useState(initialKeyword);
	const [debouncedKeyword, setDebouncedKeyword] = useState(initialKeyword);
	const [roleFilter, setRoleFilter] = useState<"__all__" | UserRole>(
		initialRole === "admin" || initialRole === "user" ? initialRole : "__all__",
	);
	const [statusFilter, setStatusFilter] = useState<"__all__" | UserStatus>(
		initialStatus === "active" || initialStatus === "disabled"
			? initialStatus
			: "__all__",
	);
	const [detailDialogUserId, setDetailDialogUserId] = useState<number | null>(
		null,
	);
	const [deleteUserId, setDeleteUserId] = useState<number | null>(null);
	const [createDialogOpen, setCreateDialogOpen] = useState(false);
	const [creating, setCreating] = useState(false);
	const [createErrors, setCreateErrors] = useState<Partial<CreateUserReq>>({});
	const [createForm, setCreateForm] = useState<CreateUserReq>({
		username: "",
		email: "",
		password: "",
	});

	useEffect(() => {
		const timer = window.setTimeout(() => {
			setDebouncedKeyword(keyword);
		}, 300);
		return () => window.clearTimeout(timer);
	}, [keyword]);

	useEffect(() => {
		const params = new URLSearchParams();
		if (debouncedKeyword.trim()) params.set("keyword", debouncedKeyword.trim());
		if (roleFilter !== "__all__") params.set("role", roleFilter);
		if (statusFilter !== "__all__") params.set("status", statusFilter);
		if (offset > 0) params.set("offset", String(offset));
		if (pageSize !== 20) params.set("pageSize", String(pageSize));
		setSearchParams(params, { replace: true });
	}, [
		debouncedKeyword,
		offset,
		pageSize,
		roleFilter,
		setSearchParams,
		statusFilter,
	]);

	const load = useCallback(async () => {
		try {
			setLoading(true);
			const page = await adminUserService.list({
				limit: pageSize,
				offset,
				keyword: debouncedKeyword.trim() || undefined,
				role: roleFilter === "__all__" ? undefined : roleFilter,
				status: statusFilter === "__all__" ? undefined : statusFilter,
			});
			setUsers(page.items);
			setTotal(page.total);
		} catch (e) {
			handleApiError(e);
		} finally {
			setLoading(false);
		}
	}, [debouncedKeyword, offset, pageSize, roleFilter, statusFilter]);

	useEffect(() => {
		void load();
	}, [load]);

	const activeFilterCount =
		(debouncedKeyword.trim().length > 0 ? 1 : 0) +
		(roleFilter !== "__all__" ? 1 : 0) +
		(statusFilter !== "__all__" ? 1 : 0);
	const totalPages = Math.max(1, Math.ceil(total / pageSize));
	const currentPage = Math.floor(offset / pageSize) + 1;
	const hasServerFilters = activeFilterCount > 0;
	const prevPageDisabled = offset === 0;
	const nextPageDisabled = offset + pageSize >= total;

	const resetFilters = () => {
		setKeyword("");
		setRoleFilter("__all__");
		setStatusFilter("__all__");
		setOffset(0);
	};

	const handlePageSizeChange = (value: string | null) => {
		if (!value) return;
		const next = Number(value) as (typeof USER_PAGE_SIZE_OPTIONS)[number];
		setPageSize(next);
		setOffset(0);
	};

	const handleKeywordChange = (value: string) => {
		setKeyword(value);
		setOffset(0);
	};

	const handleRoleFilterChange = (value: string | null) => {
		if (!value) return;
		setRoleFilter(value as "__all__" | UserRole);
		setOffset(0);
	};

	const handleStatusFilterChange = (value: string | null) => {
		if (!value) return;
		setStatusFilter(value as "__all__" | UserStatus);
		setOffset(0);
	};

	const resetCreateForm = () => {
		setCreateForm({ username: "", email: "", password: "" });
		setCreateErrors({});
	};

	const validateCreateField = (field: keyof CreateUserReq, value: string) => {
		const schema =
			field === "username"
				? createUserUsernameSchema
				: field === "email"
					? createUserEmailSchema
					: createUserPasswordSchema;
		const result = schema.safeParse(value);
		setCreateErrors((prev) => {
			if (result.success) {
				const next = { ...prev };
				delete next[field];
				return next;
			}
			return { ...prev, [field]: result.error.issues[0]?.message ?? "" };
		});
	};

	const validateCreateForm = () => {
		const nextErrors: Partial<CreateUserReq> = {};
		const usernameResult = createUserUsernameSchema.safeParse(
			createForm.username.trim(),
		);
		if (!usernameResult.success) {
			nextErrors.username = usernameResult.error.issues[0]?.message ?? "";
		}
		const emailResult = createUserEmailSchema.safeParse(
			createForm.email.trim(),
		);
		if (!emailResult.success) {
			nextErrors.email = emailResult.error.issues[0]?.message ?? "";
		}
		const passwordResult = createUserPasswordSchema.safeParse(
			createForm.password,
		);
		if (!passwordResult.success) {
			nextErrors.password = passwordResult.error.issues[0]?.message ?? "";
		}
		setCreateErrors(nextErrors);
		return Object.keys(nextErrors).length === 0;
	};

	const handleCreateFormChange = (key: keyof CreateUserReq, value: string) => {
		setCreateForm((prev) => ({ ...prev, [key]: value }));
	};

	const handleCreateUser = async (event: FormEvent<HTMLFormElement>) => {
		event.preventDefault();
		if (!validateCreateForm()) return;
		try {
			setCreating(true);
			await adminUserService.create({
				username: createForm.username.trim(),
				email: createForm.email.trim(),
				password: createForm.password,
			});
			toast.success(t("user_created"));
			setCreateDialogOpen(false);
			resetCreateForm();
			await load();
		} catch (e) {
			handleApiError(e);
		} finally {
			setCreating(false);
		}
	};

	const updateRole = async (id: number, role: UserRole) => {
		try {
			const updated = await adminUserService.update(id, { role });
			setUsers((prev) => prev.map((u) => (u.id === id ? updated : u)));
			toast.success(t("role_updated"));
		} catch (e) {
			handleApiError(e);
		}
	};

	const updateStatus = async (id: number, status: UserStatus) => {
		try {
			const updated = await adminUserService.update(id, { status });
			setUsers((prev) => prev.map((u) => (u.id === id ? updated : u)));
			toast.success(t("status_updated"));
		} catch (e) {
			handleApiError(e);
		}
	};

	const updateQuota = async (id: number, storage_quota: number) => {
		try {
			const updated = await adminUserService.update(id, {
				storage_quota,
			});
			setUsers((prev) => prev.map((u) => (u.id === id ? updated : u)));
			toast.success(t("quota_updated"));
		} catch (e) {
			handleApiError(e);
		}
	};

	const deleteUser = async (id: number) => {
		try {
			await adminUserService.delete(id);
			const isLastItemOnPage = users.length === 1;
			const nextOffset =
				isLastItemOnPage && offset > 0
					? Math.max(0, offset - pageSize)
					: offset;
			if (detailDialogUserId === id) {
				setDetailDialogUserId(null);
			}
			if (nextOffset !== offset) {
				setOffset(nextOffset);
			} else {
				setUsers((prev) => prev.filter((u) => u.id !== id));
				setTotal((prev) => Math.max(0, prev - 1));
			}
			toast.success(t("user_deleted"));
		} catch (e) {
			handleApiError(e);
		}
	};

	const selectedUser = useMemo(
		() => users.find((user) => user.id === detailDialogUserId) ?? null,
		[users, detailDialogUserId],
	);
	const deleteTargetUser = useMemo(
		() => users.find((user) => user.id === deleteUserId) ?? null,
		[users, deleteUserId],
	);

	return (
		<AdminLayout>
			<AdminPageShell>
				<AdminPageHeader
					title={t("users")}
					description={t("users_intro")}
					actions={
						<>
							<Button
								size="sm"
								className={ADMIN_CONTROL_HEIGHT_CLASS}
								onClick={() => setCreateDialogOpen(true)}
							>
								<Icon name="Plus" className="mr-1 h-4 w-4" />
								{t("new_user")}
							</Button>
							<Button
								variant="outline"
								size="sm"
								className={ADMIN_CONTROL_HEIGHT_CLASS}
								onClick={() => void load()}
								disabled={loading}
							>
								<Icon
									name={loading ? "Spinner" : "ArrowsClockwise"}
									className={`mr-1 h-3.5 w-3.5 ${loading ? "animate-spin" : ""}`}
								/>
								{t("refresh")}
							</Button>
						</>
					}
					toolbar={
						<>
							<div className="relative min-w-[240px] flex-1 md:max-w-sm">
								<Icon
									name="MagnifyingGlass"
									className="pointer-events-none absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2 text-muted-foreground"
								/>
								<Input
									value={keyword}
									onChange={(e) => handleKeywordChange(e.target.value)}
									placeholder={t("user_search_placeholder")}
									className={`${ADMIN_CONTROL_HEIGHT_CLASS} pl-9`}
								/>
							</div>
							<Select value={roleFilter} onValueChange={handleRoleFilterChange}>
								<SelectTrigger
									className={`${ADMIN_CONTROL_HEIGHT_CLASS} w-[140px]`}
								>
									<SelectValue />
								</SelectTrigger>
								<SelectContent>
									<SelectItem value="__all__">{t("all_roles")}</SelectItem>
									<SelectItem value="admin">Admin</SelectItem>
									<SelectItem value="user">User</SelectItem>
								</SelectContent>
							</Select>
							<Select
								value={statusFilter}
								onValueChange={handleStatusFilterChange}
							>
								<SelectTrigger
									className={`${ADMIN_CONTROL_HEIGHT_CLASS} w-[150px]`}
								>
									<SelectValue />
								</SelectTrigger>
								<SelectContent>
									<SelectItem value="__all__">{t("all_statuses")}</SelectItem>
									<SelectItem value="active">{t("common:active")}</SelectItem>
									<SelectItem value="disabled">
										{t("common:disabled_status")}
									</SelectItem>
								</SelectContent>
							</Select>
							<div className="ml-auto flex items-center gap-2 text-xs text-muted-foreground">
								{hasServerFilters ? <span>{t("filters_active")}</span> : null}
								{activeFilterCount > 0 ? (
									<Button
										variant="ghost"
										size="sm"
										className={ADMIN_CONTROL_HEIGHT_CLASS}
										onClick={resetFilters}
									>
										{t("clear_filters")}
									</Button>
								) : null}
							</div>
						</>
					}
				/>
				{loading ? (
					<SkeletonTable columns={7} rows={6} />
				) : users.length === 0 ? (
					hasServerFilters ? (
						<EmptyState
							icon={<Icon name="ListBullets" className="h-10 w-10" />}
							title={t("no_filtered_users")}
							description={t("no_filtered_users_desc")}
							action={
								<Button variant="outline" onClick={resetFilters}>
									{t("clear_filters")}
								</Button>
							}
						/>
					) : (
						<EmptyState title={t("no_users")} />
					)
				) : (
					<AdminSurface>
						<ScrollArea className="min-h-0 flex-1">
							<Table>
								<TableHeader>
									<TableRow>
										<TableHead className="w-16">{t("id")}</TableHead>
										<TableHead>{t("username")}</TableHead>
										<TableHead>{t("email")}</TableHead>
										<TableHead className="w-32">{t("role")}</TableHead>
										<TableHead className="w-32">{t("common:status")}</TableHead>
										<TableHead className="w-[220px]">{t("storage")}</TableHead>
										<TableHead className="w-20">
											{t("common:actions")}
										</TableHead>
									</TableRow>
								</TableHeader>
								<TableBody>
									{users.map((user) => (
										<TableRow key={user.id}>
											<TableCell className="font-mono text-xs text-muted-foreground">
												{user.id}
											</TableCell>
											<TableCell>
												<button
													type="button"
													className="flex w-full min-w-0 items-center rounded-lg border border-transparent bg-muted/10 px-2 py-2 text-left transition-colors duration-200 hover:border-border hover:bg-muted/25 focus-visible:border-ring focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
													onClick={() => setDetailDialogUserId(user.id)}
													title={t("view_details")}
												>
													<div className="min-w-0">
														<div className="truncate font-medium text-foreground">
															{user.username}
														</div>
													</div>
												</button>
											</TableCell>
											<TableCell>
												<button
													type="button"
													className="flex w-full min-w-0 items-center rounded-lg border border-transparent bg-muted/10 px-2 py-2 text-left transition-colors duration-200 hover:border-border hover:bg-muted/25 focus-visible:border-ring focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
													onClick={() => setDetailDialogUserId(user.id)}
													title={t("view_details")}
												>
													<div className="truncate text-sm text-muted-foreground">
														{user.email}
													</div>
												</button>
											</TableCell>
											<TableCell>
												<button
													type="button"
													className="flex w-full items-center rounded-lg border border-transparent bg-muted/20 px-3 py-2 text-left transition-colors duration-200 hover:border-border hover:bg-muted/35 focus-visible:border-ring focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
													onClick={() => setDetailDialogUserId(user.id)}
													title={t("view_details")}
												>
													<Badge
														variant="outline"
														className={getRoleBadgeClass(user.role)}
													>
														{user.role === "admin" ? "Admin" : "User"}
													</Badge>
												</button>
											</TableCell>
											<TableCell>
												<button
													type="button"
													className="flex w-full items-center rounded-lg border border-transparent bg-muted/20 px-3 py-2 text-left transition-colors duration-200 hover:border-border hover:bg-muted/35 focus-visible:border-ring focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
													onClick={() => setDetailDialogUserId(user.id)}
													title={t("view_details")}
												>
													<Badge
														variant="outline"
														className={getStatusBadgeClass(user.status)}
													>
														{user.status === "active"
															? t("common:active")
															: t("common:disabled_status")}
													</Badge>
												</button>
											</TableCell>
											<TableCell>
												<button
													type="button"
													className="w-full text-left"
													onClick={() => setDetailDialogUserId(user.id)}
													title={t("view_details")}
												>
													<QuotaCell user={user} />
												</button>
											</TableCell>
											<TableCell>
												<div className="flex justify-end">
													<TooltipProvider>
														<Tooltip>
															<TooltipTrigger>
																<div>
																	<Button
																		variant="ghost"
																		size="icon"
																		className={`${ADMIN_ICON_BUTTON_CLASS} text-destructive`}
																		onClick={() => setDeleteUserId(user.id)}
																		aria-label={t("delete_user")}
																		title={t("delete_user")}
																		disabled={user.id === 1}
																	>
																		<Icon
																			name="Trash"
																			className="h-3.5 w-3.5"
																		/>
																	</Button>
																</div>
															</TooltipTrigger>
															{user.id === 1 ? (
																<TooltipContent>
																	{t("initial_admin_delete_blocked")}
																</TooltipContent>
															) : null}
														</Tooltip>
													</TooltipProvider>
												</div>
											</TableCell>
										</TableRow>
									))}
								</TableBody>
							</Table>
						</ScrollArea>
					</AdminSurface>
				)}

				{total > 0 ? (
					<div className="flex items-center justify-between gap-3 px-4 pb-4 text-sm text-muted-foreground md:px-6">
						<div className="flex items-center gap-3">
							<span>
								{t("entries_page", {
									total,
									current: currentPage,
									pages: totalPages,
								})}
							</span>
							<Select
								value={String(pageSize)}
								onValueChange={handlePageSizeChange}
							>
								<SelectTrigger
									className={`${ADMIN_CONTROL_HEIGHT_CLASS} w-[120px]`}
								>
									<SelectValue />
								</SelectTrigger>
								<SelectContent>
									{USER_PAGE_SIZE_OPTIONS.map((size) => (
										<SelectItem key={size} value={String(size)}>
											{t("page_size_option", { count: size })}
										</SelectItem>
									))}
								</SelectContent>
							</Select>
						</div>
						<TooltipProvider>
							<div className="flex items-center gap-2">
								<Tooltip>
									<TooltipTrigger
										render={
											<Button
												variant="outline"
												size="sm"
												disabled={prevPageDisabled}
												onClick={() =>
													setOffset(Math.max(0, offset - pageSize))
												}
											/>
										}
									>
										<Icon name="CaretLeft" className="h-4 w-4" />
									</TooltipTrigger>
									{prevPageDisabled ? (
										<TooltipContent>
											{t("pagination_prev_disabled")}
										</TooltipContent>
									) : null}
								</Tooltip>
								<Tooltip>
									<TooltipTrigger
										render={
											<Button
												variant="outline"
												size="sm"
												disabled={nextPageDisabled}
												onClick={() => setOffset(offset + pageSize)}
											/>
										}
									>
										<Icon name="CaretRight" className="h-4 w-4" />
									</TooltipTrigger>
									{nextPageDisabled ? (
										<TooltipContent>
											{t("pagination_next_disabled")}
										</TooltipContent>
									) : null}
								</Tooltip>
							</div>
						</TooltipProvider>
					</div>
				) : null}
			</AdminPageShell>
			<Dialog
				open={createDialogOpen}
				onOpenChange={(open) => {
					setCreateDialogOpen(open);
					if (!open && !creating) {
						resetCreateForm();
					}
				}}
			>
				<DialogContent className="sm:max-w-md">
					<form onSubmit={handleCreateUser} className="space-y-4">
						<DialogHeader>
							<DialogTitle>{t("create_user")}</DialogTitle>
							<DialogDescription>{t("create_user_desc")}</DialogDescription>
						</DialogHeader>
						<div className="space-y-2">
							<Label htmlFor="create-user-username">{t("username")}</Label>
							<Input
								id="create-user-username"
								value={createForm.username}
								onChange={(e) => {
									const value = e.target.value;
									handleCreateFormChange("username", value);
									validateCreateField("username", value.trim());
								}}
								required
								className={ADMIN_CONTROL_HEIGHT_CLASS}
								aria-invalid={!!createErrors.username}
							/>
							{createErrors.username ? (
								<p className="text-xs text-destructive">
									{createErrors.username}
								</p>
							) : null}
						</div>
						<div className="space-y-2">
							<Label htmlFor="create-user-email">{t("email")}</Label>
							<Input
								id="create-user-email"
								value={createForm.email}
								onChange={(e) => {
									const value = e.target.value;
									handleCreateFormChange("email", value);
									validateCreateField("email", value.trim());
								}}
								required
								className={ADMIN_CONTROL_HEIGHT_CLASS}
								aria-invalid={!!createErrors.email}
							/>
							{createErrors.email ? (
								<p className="text-xs text-destructive">{createErrors.email}</p>
							) : null}
						</div>
						<div className="space-y-2">
							<Label htmlFor="create-user-password">{t("password")}</Label>
							<Input
								id="create-user-password"
								type="password"
								value={createForm.password}
								onChange={(e) => {
									const value = e.target.value;
									handleCreateFormChange("password", value);
									validateCreateField("password", value);
								}}
								required
								className={ADMIN_CONTROL_HEIGHT_CLASS}
								aria-invalid={!!createErrors.password}
							/>
							{createErrors.password ? (
								<p className="text-xs text-destructive">
									{createErrors.password}
								</p>
							) : null}
						</div>
						<DialogFooter>
							<Button
								type="button"
								variant="outline"
								onClick={() => setCreateDialogOpen(false)}
								disabled={creating}
							>
								{t("common:cancel")}
							</Button>
							<Button type="submit" disabled={creating}>
								{creating ? (
									<Icon name="Spinner" className="mr-1 h-4 w-4 animate-spin" />
								) : (
									<Icon name="Plus" className="mr-1 h-4 w-4" />
								)}
								{t("common:create")}
							</Button>
						</DialogFooter>
					</form>
				</DialogContent>
			</Dialog>
			<UserDetailDialog
				user={selectedUser}
				open={detailDialogUserId !== null}
				onOpenChange={(open) => {
					if (!open) setDetailDialogUserId(null);
				}}
				onUpdateRole={updateRole}
				onUpdateStatus={updateStatus}
				onUpdateQuota={updateQuota}
			/>
			<ConfirmDialog
				open={deleteUserId !== null}
				onOpenChange={(open) => {
					if (!open) setDeleteUserId(null);
				}}
				title={t("delete_user")}
				description={
					deleteTargetUser?.id === 1
						? t("initial_admin_delete_blocked")
						: t("confirm_force_delete")
				}
				confirmLabel={t("common:delete")}
				onConfirm={() => {
					const id = deleteUserId;
					setDeleteUserId(null);
					if (id !== null && id !== 1) {
						void deleteUser(id);
					}
				}}
				variant="destructive"
			/>
		</AdminLayout>
	);
}
