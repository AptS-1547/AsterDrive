import type { FormEvent } from "react";
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { useSearchParams } from "react-router-dom";
import { toast } from "sonner";
import { UserDetailDialog } from "@/components/admin/UserDetailDialog";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
import { EmptyState } from "@/components/common/EmptyState";
import { SkeletonTable } from "@/components/common/SkeletonTable";
import { UserAvatarImage } from "@/components/common/UserAvatarImage";
import {
	getRoleBadgeClass,
	getStatusBadgeClass,
} from "@/components/common/UserStatusBadge";
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
import { useApiList } from "@/hooks/useApiList";
import {
	ADMIN_CONTROL_HEIGHT_CLASS,
	ADMIN_ICON_BUTTON_CLASS,
} from "@/lib/constants";
import { formatBytes } from "@/lib/format";
import {
	buildOffsetPaginationSearchParams,
	parseOffsetSearchParam,
	parsePageSizeOption,
	parsePageSizeSearchParam,
} from "@/lib/pagination";
import { getNormalizedDisplayName, getUserDisplayName } from "@/lib/user";
import { emailSchema, passwordSchema, usernameSchema } from "@/lib/validation";
import { adminUserService } from "@/services/adminService";
import type {
	CreateUserReq,
	UpdateUserRequest,
	UserInfo,
	UserRole,
	UserStatus,
} from "@/types/api";

const USER_PAGE_SIZE_OPTIONS = [10, 20, 50] as const;
const DEFAULT_USER_PAGE_SIZE = 20 as const;
const INTERACTIVE_TABLE_ROW_CLASS =
	"cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/50";
const USER_TEXT_CELL_CONTENT_CLASS =
	"flex min-w-0 items-center rounded-lg bg-muted/10 px-3 py-3 text-left transition-colors duration-200";
const USER_BADGE_CELL_CONTENT_CLASS =
	"flex items-center rounded-lg bg-muted/20 px-3 py-3 text-left transition-colors duration-200";

function normalizeOffset(offset: number) {
	return Math.max(0, Math.floor(offset));
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
					{quota > 0 ? ` / ${formatBytes(quota)}` : ` / ${t("core:unlimited")}`}
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
	const [offset, setOffsetState] = useState(
		normalizeOffset(parseOffsetSearchParam(searchParams.get("offset"))),
	);
	const [pageSize, setPageSize] = useState<
		(typeof USER_PAGE_SIZE_OPTIONS)[number]
	>(
		parsePageSizeSearchParam(
			searchParams.get("pageSize"),
			USER_PAGE_SIZE_OPTIONS,
			DEFAULT_USER_PAGE_SIZE,
		),
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
	const setOffset = (value: number) => {
		setOffsetState(normalizeOffset(value));
	};

	useEffect(() => {
		const timer = window.setTimeout(() => {
			setDebouncedKeyword(keyword);
		}, 300);
		return () => window.clearTimeout(timer);
	}, [keyword]);

	useEffect(() => {
		setSearchParams(
			buildOffsetPaginationSearchParams({
				offset,
				pageSize,
				defaultPageSize: DEFAULT_USER_PAGE_SIZE,
				extraParams: {
					keyword: debouncedKeyword.trim() || undefined,
					role: roleFilter !== "__all__" ? roleFilter : undefined,
					status: statusFilter !== "__all__" ? statusFilter : undefined,
				},
			}),
			{ replace: true },
		);
	}, [
		debouncedKeyword,
		offset,
		pageSize,
		roleFilter,
		setSearchParams,
		statusFilter,
	]);

	const {
		items: users,
		loading,
		reload: reloadUsers,
		setItems: setUsers,
		setTotal,
		total,
	} = useApiList(
		() =>
			adminUserService.list({
				limit: pageSize,
				offset,
				keyword: debouncedKeyword.trim() || undefined,
				role: roleFilter === "__all__" ? undefined : roleFilter,
				status: statusFilter === "__all__" ? undefined : statusFilter,
			}),
		[debouncedKeyword, offset, pageSize, roleFilter, statusFilter],
	);

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
		setDebouncedKeyword("");
		setRoleFilter("__all__");
		setStatusFilter("__all__");
		setOffset(0);
	};

	const handlePageSizeChange = (value: string | null) => {
		const next = parsePageSizeOption(value, USER_PAGE_SIZE_OPTIONS);
		if (next == null) return;
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
				? usernameSchema
				: field === "email"
					? emailSchema
					: passwordSchema;
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
		const usernameResult = usernameSchema.safeParse(createForm.username.trim());
		if (!usernameResult.success) {
			nextErrors.username = usernameResult.error.issues[0]?.message ?? "";
		}
		const emailResult = emailSchema.safeParse(createForm.email.trim());
		if (!emailResult.success) {
			nextErrors.email = emailResult.error.issues[0]?.message ?? "";
		}
		const passwordResult = passwordSchema.safeParse(createForm.password);
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
			await reloadUsers();
		} catch (e) {
			handleApiError(e);
		} finally {
			setCreating(false);
		}
	};

	const updateUser = async (id: number, data: UpdateUserRequest) => {
		try {
			await adminUserService.update(id, data);
			await reloadUsers();
			toast.success(t("user_updated"));
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
	const roleFilterOptions = [
		{ label: t("all_roles"), value: "__all__" },
		{ label: t("role_admin"), value: "admin" },
		{ label: t("role_user"), value: "user" },
	] satisfies ReadonlyArray<{ label: string; value: string }>;
	const statusFilterOptions = [
		{ label: t("all_statuses"), value: "__all__" },
		{ label: t("core:active"), value: "active" },
		{ label: t("core:disabled_status"), value: "disabled" },
	] satisfies ReadonlyArray<{ label: string; value: string }>;
	const pageSizeOptions = USER_PAGE_SIZE_OPTIONS.map((size) => ({
		label: t("page_size_option", { count: size }),
		value: String(size),
	}));

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
								onClick={() => void reloadUsers()}
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
							<Select
								items={roleFilterOptions}
								value={roleFilter}
								onValueChange={handleRoleFilterChange}
							>
								<SelectTrigger
									className={`${ADMIN_CONTROL_HEIGHT_CLASS} w-[140px]`}
								>
									<SelectValue />
								</SelectTrigger>
								<SelectContent>
									{roleFilterOptions.map((option) => (
										<SelectItem key={option.value} value={option.value}>
											{option.label}
										</SelectItem>
									))}
								</SelectContent>
							</Select>
							<Select
								items={statusFilterOptions}
								value={statusFilter}
								onValueChange={handleStatusFilterChange}
							>
								<SelectTrigger
									className={`${ADMIN_CONTROL_HEIGHT_CLASS} w-[150px]`}
								>
									<SelectValue />
								</SelectTrigger>
								<SelectContent>
									{statusFilterOptions.map((option) => (
										<SelectItem key={option.value} value={option.value}>
											{option.label}
										</SelectItem>
									))}
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
					<AdminSurface padded={false}>
						<ScrollArea className="min-h-0 flex-1">
							<Table>
								<TableHeader>
									<TableRow>
										<TableHead className="w-16">{t("id")}</TableHead>
										<TableHead>{t("username")}</TableHead>
										<TableHead>{t("email")}</TableHead>
										<TableHead className="w-32">{t("role")}</TableHead>
										<TableHead className="w-32">{t("core:status")}</TableHead>
										<TableHead className="w-[220px]">{t("storage")}</TableHead>
										<TableHead className="w-20">{t("core:actions")}</TableHead>
									</TableRow>
								</TableHeader>
								<TableBody>
									{users.map((user) => (
										<TableRow
											key={user.id}
											className={INTERACTIVE_TABLE_ROW_CLASS}
											onClick={() => setDetailDialogUserId(user.id)}
											onKeyDown={(event) => {
												if (event.key === "Enter" || event.key === " ") {
													event.preventDefault();
													setDetailDialogUserId(user.id);
												}
											}}
											tabIndex={0}
										>
											<TableCell className="font-mono text-xs text-muted-foreground">
												{user.id}
											</TableCell>
											<TableCell>
												<div className={USER_TEXT_CELL_CONTENT_CLASS}>
													<UserAvatarImage
														avatar={user.profile.avatar}
														name={getUserDisplayName(user)}
														alt=""
														size="sm"
														className="mr-3 h-7 w-7 rounded-lg text-[11px]"
													/>
													<div className="min-w-0">
														<div className="truncate font-medium text-foreground">
															{getUserDisplayName(user)}
														</div>
														{getNormalizedDisplayName(
															user.profile.display_name,
														) && getUserDisplayName(user) !== user.username ? (
															<div className="truncate text-xs text-muted-foreground">
																@{user.username}
															</div>
														) : null}
													</div>
												</div>
											</TableCell>
											<TableCell>
												<div className={USER_TEXT_CELL_CONTENT_CLASS}>
													<div className="truncate text-sm text-muted-foreground">
														{user.email}
													</div>
												</div>
											</TableCell>
											<TableCell>
												<div className={USER_BADGE_CELL_CONTENT_CLASS}>
													<Badge
														variant="outline"
														className={getRoleBadgeClass(user.role)}
													>
														{user.role === "admin" ? "Admin" : "User"}
													</Badge>
												</div>
											</TableCell>
											<TableCell>
												<div className={USER_BADGE_CELL_CONTENT_CLASS}>
													<Badge
														variant="outline"
														className={getStatusBadgeClass(user.status)}
													>
														{user.status === "active"
															? t("core:active")
															: t("core:disabled_status")}
													</Badge>
												</div>
											</TableCell>
											<TableCell>
												<div className="w-full text-left">
													<QuotaCell user={user} />
												</div>
											</TableCell>
											<TableCell
												onClick={(event) => event.stopPropagation()}
												onKeyDown={(event) => event.stopPropagation()}
											>
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
								items={pageSizeOptions}
								value={String(pageSize)}
								onValueChange={handlePageSizeChange}
							>
								<SelectTrigger
									className={`${ADMIN_CONTROL_HEIGHT_CLASS} w-[120px]`}
								>
									<SelectValue />
								</SelectTrigger>
								<SelectContent>
									{pageSizeOptions.map((option) => (
										<SelectItem key={option.value} value={option.value}>
											{option.label}
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
					<form
						onSubmit={handleCreateUser}
						autoComplete="off"
						className="space-y-4"
					>
						<DialogHeader>
							<DialogTitle>{t("create_user")}</DialogTitle>
							<DialogDescription>{t("create_user_desc")}</DialogDescription>
						</DialogHeader>
						<div className="space-y-2">
							<Label htmlFor="create-user-username">{t("username")}</Label>
							<Input
								id="create-user-username"
								name="admin-create-user-username"
								value={createForm.username}
								onChange={(e) => {
									const value = e.target.value;
									handleCreateFormChange("username", value);
									validateCreateField("username", value.trim());
								}}
								autoComplete="off"
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
								name="admin-create-user-email"
								value={createForm.email}
								onChange={(e) => {
									const value = e.target.value;
									handleCreateFormChange("email", value);
									validateCreateField("email", value.trim());
								}}
								autoComplete="off"
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
								name="admin-create-user-password"
								type="password"
								value={createForm.password}
								onChange={(e) => {
									const value = e.target.value;
									handleCreateFormChange("password", value);
									validateCreateField("password", value);
								}}
								autoComplete="new-password"
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
								{t("core:cancel")}
							</Button>
							<Button type="submit" disabled={creating}>
								{creating ? (
									<Icon name="Spinner" className="mr-1 h-4 w-4 animate-spin" />
								) : (
									<Icon name="Plus" className="mr-1 h-4 w-4" />
								)}
								{t("core:create")}
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
				onUpdate={updateUser}
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
				confirmLabel={t("core:delete")}
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
