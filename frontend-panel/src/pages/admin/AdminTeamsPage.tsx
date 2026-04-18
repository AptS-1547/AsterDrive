import {
	type FormEvent,
	useCallback,
	useEffect,
	useRef,
	useState,
} from "react";
import { useTranslation } from "react-i18next";
import { useNavigate, useSearchParams } from "react-router-dom";
import { toast } from "sonner";
import { AdminOffsetPagination } from "@/components/admin/AdminOffsetPagination";
import {
	CreateTeamDialog,
	type CreateTeamFormState,
	type TeamPolicyGroupOption,
} from "@/components/admin/admin-teams-page/CreateTeamDialog";
import { EmptyState } from "@/components/common/EmptyState";
import { SkeletonTable } from "@/components/common/SkeletonTable";
import { AdminLayout } from "@/components/layout/AdminLayout";
import { AdminPageHeader } from "@/components/layout/AdminPageHeader";
import { AdminPageShell } from "@/components/layout/AdminPageShell";
import { AdminSurface } from "@/components/layout/AdminSurface";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/icon";
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
import { useApiList } from "@/hooks/useApiList";
import { usePageTitle } from "@/hooks/usePageTitle";
import {
	loadAdminPolicyGroupLookup,
	readAdminPolicyGroupLookup,
} from "@/lib/adminPolicyGroupLookup";
import {
	ADMIN_CONTROL_HEIGHT_CLASS,
	ADMIN_ICON_BUTTON_CLASS,
	ADMIN_TABLE_ACTIONS_WIDTH_CLASS,
} from "@/lib/constants";
import { formatBytes, formatDateShort } from "@/lib/format";
import {
	buildOffsetPaginationSearchParams,
	parseOffsetSearchParam,
	parsePageSizeOption,
	parsePageSizeSearchParam,
} from "@/lib/pagination";
import { adminTeamService } from "@/services/adminService";
import type { AdminTeamInfo, StoragePolicyGroup } from "@/types/api";

const TEAM_PAGE_SIZE_OPTIONS = [10, 20, 50] as const;
const DEFAULT_TEAM_PAGE_SIZE = 20 as const;
const TEAM_MANAGED_QUERY_KEYS = [
	"archived",
	"keyword",
	"offset",
	"pageSize",
] as const;

const EMPTY_CREATE_FORM: CreateTeamFormState = {
	name: "",
	description: "",
	adminIdentifier: "",
	policyGroupId: "",
};

const INTERACTIVE_TABLE_ROW_CLASS =
	"cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/50";
const TEAM_TEXT_CELL_CONTENT_CLASS =
	"flex min-w-0 items-center rounded-lg bg-muted/10 px-3 py-3 text-left transition-colors duration-200";
const TEAM_BADGE_CELL_CONTENT_CLASS =
	"flex items-center rounded-lg bg-muted/20 px-3 py-3 text-left transition-colors duration-200";

function normalizeOffset(offset: number) {
	return Math.max(0, Math.floor(offset));
}

function parseArchivedSearchParam(value: string | null) {
	return value === "1" || value === "true";
}

function buildManagedTeamSearchParams({
	offset,
	pageSize,
	keyword,
	archived,
}: {
	offset: number;
	pageSize: (typeof TEAM_PAGE_SIZE_OPTIONS)[number];
	keyword: string;
	archived: boolean;
}) {
	return buildOffsetPaginationSearchParams({
		offset,
		pageSize,
		defaultPageSize: DEFAULT_TEAM_PAGE_SIZE,
		extraParams: {
			archived: archived ? true : undefined,
			keyword: keyword.trim() || undefined,
		},
	});
}

function getManagedTeamSearchString(searchParams: URLSearchParams) {
	return buildManagedTeamSearchParams({
		offset: normalizeOffset(parseOffsetSearchParam(searchParams.get("offset"))),
		pageSize: parsePageSizeSearchParam(
			searchParams.get("pageSize"),
			TEAM_PAGE_SIZE_OPTIONS,
			DEFAULT_TEAM_PAGE_SIZE,
		),
		keyword: searchParams.get("keyword") ?? "",
		archived: parseArchivedSearchParam(searchParams.get("archived")),
	}).toString();
}

function mergeManagedTeamSearchParams(
	searchParams: URLSearchParams,
	managedSearchParams: URLSearchParams,
) {
	const merged = new URLSearchParams(searchParams);
	for (const key of TEAM_MANAGED_QUERY_KEYS) {
		merged.delete(key);
	}
	for (const [key, value] of managedSearchParams.entries()) {
		merged.set(key, value);
	}
	return merged;
}

function getDefaultPolicyGroupId(policyGroups: StoragePolicyGroup[]) {
	return (
		policyGroups.find(
			(group) => group.is_default && group.is_enabled && group.items.length > 0,
		)?.id ??
		policyGroups.find((group) => group.is_enabled && group.items.length > 0)
			?.id ??
		null
	);
}

function buildPolicyGroupOptions(
	policyGroups: StoragePolicyGroup[],
	selectedPolicyGroupId: number | null,
): TeamPolicyGroupOption[] {
	const options: TeamPolicyGroupOption[] = policyGroups
		.filter((group) => group.is_enabled && group.items.length > 0)
		.map((group) => ({
			label: group.name,
			value: String(group.id),
		}));

	if (
		selectedPolicyGroupId != null &&
		!options.some((option) => option.value === String(selectedPolicyGroupId))
	) {
		const selectedGroup = policyGroups.find(
			(group) => group.id === selectedPolicyGroupId,
		);
		options.unshift({
			label: selectedGroup?.name ?? `#${selectedPolicyGroupId}`,
			value: String(selectedPolicyGroupId),
			disabled: true,
		});
	}

	return options;
}

function TeamStorageCell({
	team,
	policyGroupName,
}: {
	team: AdminTeamInfo;
	policyGroupName: string | null;
}) {
	const { t } = useTranslation(["admin", "core"]);

	return (
		<div className="flex min-w-0 flex-col gap-1 rounded-lg bg-muted/10 px-3 py-3 text-left">
			<span className="text-sm font-medium text-foreground">
				{formatBytes(team.storage_used)}
				{team.storage_quota > 0
					? ` / ${formatBytes(team.storage_quota)}`
					: ` / ${t("core:unlimited")}`}
			</span>
			<span className="truncate text-xs text-muted-foreground">
				#{team.id}
				{team.policy_group_id != null
					? ` · ${policyGroupName ?? `PG ${team.policy_group_id}`}`
					: ""}
			</span>
		</div>
	);
}

export default function AdminTeamsPage() {
	const { t } = useTranslation(["admin", "core"]);
	usePageTitle(t("teams"));
	const initialPolicyGroups = readAdminPolicyGroupLookup();
	const navigate = useNavigate();
	const [searchParams, setSearchParams] = useSearchParams();
	const initialKeyword = searchParams.get("keyword") ?? "";
	const [offset, setOffsetState] = useState(
		normalizeOffset(parseOffsetSearchParam(searchParams.get("offset"))),
	);
	const [pageSize, setPageSize] = useState<
		(typeof TEAM_PAGE_SIZE_OPTIONS)[number]
	>(
		parsePageSizeSearchParam(
			searchParams.get("pageSize"),
			TEAM_PAGE_SIZE_OPTIONS,
			DEFAULT_TEAM_PAGE_SIZE,
		),
	);
	const [keyword, setKeyword] = useState(initialKeyword);
	const [showArchived, setShowArchived] = useState(
		parseArchivedSearchParam(searchParams.get("archived")),
	);
	const [createDialogOpen, setCreateDialogOpen] = useState(false);
	const [createForm, setCreateForm] =
		useState<CreateTeamFormState>(EMPTY_CREATE_FORM);
	const [submitting, setSubmitting] = useState(false);
	const [policyGroups, setPolicyGroups] = useState<StoragePolicyGroup[]>(
		initialPolicyGroups ?? [],
	);
	const [policyGroupsLoading, setPolicyGroupsLoading] = useState(
		initialPolicyGroups == null,
	);
	const lastWrittenSearchRef = useRef<string | null>(null);
	const setOffset = (value: number) => {
		setOffsetState(normalizeOffset(value));
	};

	useEffect(() => {
		const managedSearch = getManagedTeamSearchString(searchParams);
		if (managedSearch === lastWrittenSearchRef.current) {
			return;
		}

		const nextOffset = normalizeOffset(
			parseOffsetSearchParam(searchParams.get("offset")),
		);
		const nextPageSize = parsePageSizeSearchParam(
			searchParams.get("pageSize"),
			TEAM_PAGE_SIZE_OPTIONS,
			DEFAULT_TEAM_PAGE_SIZE,
		);
		const nextKeyword = searchParams.get("keyword") ?? "";
		const nextArchived = parseArchivedSearchParam(searchParams.get("archived"));

		setOffsetState((prev) => (prev === nextOffset ? prev : nextOffset));
		setPageSize((prev) => (prev === nextPageSize ? prev : nextPageSize));
		setKeyword((prev) => (prev === nextKeyword ? prev : nextKeyword));
		setShowArchived((prev) => (prev === nextArchived ? prev : nextArchived));
	}, [searchParams]);

	useEffect(() => {
		const nextManagedSearchParams = buildManagedTeamSearchParams({
			offset,
			pageSize,
			keyword,
			archived: showArchived,
		});
		const nextSearch = nextManagedSearchParams.toString();
		const currentSearch = getManagedTeamSearchString(searchParams);
		if (
			currentSearch !== lastWrittenSearchRef.current &&
			currentSearch !== nextSearch
		) {
			return;
		}

		lastWrittenSearchRef.current = nextSearch;
		if (nextSearch === currentSearch) {
			return;
		}

		setSearchParams(
			mergeManagedTeamSearchParams(searchParams, nextManagedSearchParams),
			{ replace: true },
		);
	}, [keyword, offset, pageSize, searchParams, setSearchParams, showArchived]);

	const {
		items: teams,
		loading,
		reload,
		total,
	} = useApiList(
		() =>
			adminTeamService.list({
				archived: showArchived,
				keyword: keyword.trim() || undefined,
				limit: pageSize,
				offset,
			}),
		[keyword, offset, pageSize, showArchived],
	);

	const loadPolicyGroups = useCallback(async () => {
		try {
			const cachedPolicyGroups = readAdminPolicyGroupLookup();
			if (cachedPolicyGroups != null) {
				setPolicyGroups(cachedPolicyGroups);
				setPolicyGroupsLoading(false);
			} else {
				setPolicyGroupsLoading(true);
			}
			setPolicyGroups(await loadAdminPolicyGroupLookup());
		} catch (error) {
			handleApiError(error);
		} finally {
			setPolicyGroupsLoading(false);
		}
	}, []);

	useEffect(() => {
		void loadPolicyGroups();
	}, [loadPolicyGroups]);

	const defaultPolicyGroupId = getDefaultPolicyGroupId(policyGroups);
	const createPolicyGroupOptions = buildPolicyGroupOptions(
		policyGroups,
		createForm.policyGroupId
			? Number(createForm.policyGroupId)
			: defaultPolicyGroupId,
	);
	const createPolicyGroupUnavailable =
		!policyGroupsLoading && createPolicyGroupOptions.length === 0;
	const activeFilterCount =
		(keyword.trim().length > 0 ? 1 : 0) + (showArchived ? 1 : 0);
	const hasServerFilters = activeFilterCount > 0;
	const totalPages = Math.max(1, Math.ceil(total / pageSize));
	const currentPage = Math.floor(offset / pageSize) + 1;
	const prevPageDisabled = offset === 0;
	const nextPageDisabled = offset + pageSize >= total;
	const pageSizeOptions = TEAM_PAGE_SIZE_OPTIONS.map((size) => ({
		label: t("page_size_option", { count: size }),
		value: String(size),
	}));

	useEffect(() => {
		if (
			createDialogOpen &&
			!createForm.policyGroupId &&
			defaultPolicyGroupId != null
		) {
			setCreateForm((prev) =>
				prev.policyGroupId
					? prev
					: { ...prev, policyGroupId: String(defaultPolicyGroupId) },
			);
		}
	}, [createDialogOpen, createForm.policyGroupId, defaultPolicyGroupId]);

	const handleOpenCreateDialog = () => {
		setCreateForm({
			...EMPTY_CREATE_FORM,
			policyGroupId:
				defaultPolicyGroupId != null ? String(defaultPolicyGroupId) : "",
		});
		setCreateDialogOpen(true);
	};

	const handleCreate = async (event: FormEvent<HTMLFormElement>) => {
		event.preventDefault();
		const name = createForm.name.trim();
		const adminIdentifier = createForm.adminIdentifier.trim();
		const policyGroupId = Number(createForm.policyGroupId);
		if (!name || !adminIdentifier || !Number.isFinite(policyGroupId)) {
			return;
		}

		try {
			setSubmitting(true);
			await adminTeamService.create({
				name,
				description: createForm.description.trim() || undefined,
				admin_identifier: adminIdentifier,
				policy_group_id: policyGroupId,
			});
			setCreateDialogOpen(false);
			setCreateForm(EMPTY_CREATE_FORM);
			toast.success(t("team_created"));
			await reload();
		} catch (error) {
			handleApiError(error);
		} finally {
			setSubmitting(false);
		}
	};

	const policyGroupNameById = (policyGroupId: number | null | undefined) =>
		policyGroupId != null
			? (policyGroups.find((group) => group.id === policyGroupId)?.name ?? null)
			: null;

	const resetFilters = () => {
		setKeyword("");
		setShowArchived(false);
		setOffset(0);
	};

	const handlePageSizeChange = (value: string | null) => {
		const next = parsePageSizeOption(value, TEAM_PAGE_SIZE_OPTIONS);
		if (next == null) return;
		setPageSize(next);
		setOffset(0);
	};

	const handleKeywordChange = (value: string) => {
		setKeyword(value);
		setOffset(0);
	};

	const handleArchivedToggle = () => {
		setShowArchived((prev) => !prev);
		setOffset(0);
	};

	return (
		<AdminLayout>
			<AdminPageShell>
				<AdminPageHeader
					title={t("teams")}
					description={t("teams_intro")}
					actions={
						<>
							<Button
								size="sm"
								className={ADMIN_CONTROL_HEIGHT_CLASS}
								onClick={handleOpenCreateDialog}
							>
								<Icon name="Plus" className="mr-1 h-4 w-4" />
								{t("new_team")}
							</Button>
							<Button
								variant="outline"
								size="sm"
								className={ADMIN_CONTROL_HEIGHT_CLASS}
								onClick={() => void reload()}
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
									onChange={(event) => handleKeywordChange(event.target.value)}
									placeholder={t("team_search_placeholder")}
									className={`${ADMIN_CONTROL_HEIGHT_CLASS} pl-9`}
								/>
							</div>
							<Button
								variant={showArchived ? "default" : "outline"}
								size="sm"
								className={ADMIN_CONTROL_HEIGHT_CLASS}
								onClick={handleArchivedToggle}
							>
								<Icon name="Cloud" className="mr-1 h-4 w-4" />
								{showArchived
									? t("show_active_teams")
									: t("show_archived_teams")}
							</Button>
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
					<SkeletonTable columns={6} rows={6} />
				) : teams.length === 0 ? (
					hasServerFilters ? (
						<EmptyState
							icon={<Icon name="Cloud" className="h-10 w-10" />}
							title={t("no_filtered_teams")}
							description={t("no_filtered_teams_desc")}
							action={
								<Button variant="outline" onClick={resetFilters}>
									{t("clear_filters")}
								</Button>
							}
						/>
					) : (
						<EmptyState
							icon={<Icon name="Cloud" className="h-10 w-10" />}
							title={t("no_teams")}
							description={t("no_teams_desc")}
						/>
					)
				) : (
					<AdminSurface padded={false}>
						<ScrollArea className="min-h-0 flex-1">
							<Table>
								<TableHeader>
									<TableRow>
										<TableHead>{t("core:name")}</TableHead>
										<TableHead>{t("created_by")}</TableHead>
										<TableHead className="w-28">{t("member_count")}</TableHead>
										<TableHead className="w-[220px]">{t("quota")}</TableHead>
										<TableHead className="w-36">
											{t("core:created_at")}
										</TableHead>
										<TableHead className={ADMIN_TABLE_ACTIONS_WIDTH_CLASS}>
											{t("core:actions")}
										</TableHead>
									</TableRow>
								</TableHeader>
								<TableBody>
									{teams.map((team) => (
										<TableRow
											key={team.id}
											className={INTERACTIVE_TABLE_ROW_CLASS}
											onClick={() =>
												navigate(`/admin/teams/${team.id}/overview`, {
													viewTransition: true,
												})
											}
											onKeyDown={(event) => {
												if (event.key === "Enter" || event.key === " ") {
													event.preventDefault();
													navigate(`/admin/teams/${team.id}/overview`, {
														viewTransition: true,
													});
												}
											}}
											tabIndex={0}
										>
											<TableCell>
												<div className="flex min-w-0 flex-col gap-2 rounded-lg bg-muted/10 px-3 py-3 text-left">
													<div className="flex flex-wrap items-center gap-2">
														<span className="font-medium text-foreground">
															{team.name}
														</span>
														<Badge variant="outline">#{team.id}</Badge>
														{team.archived_at ? (
															<Badge variant="outline">
																{t("archived_badge")}
															</Badge>
														) : null}
													</div>
													{team.description ? (
														<p className="max-w-md text-xs text-muted-foreground">
															{team.description}
														</p>
													) : null}
												</div>
											</TableCell>
											<TableCell>
												<div className="flex min-w-0 flex-col gap-1 rounded-lg bg-muted/10 px-3 py-3 text-left">
													<span className="truncate text-sm text-foreground">
														{team.created_by_username}
													</span>
													<span className="text-xs text-muted-foreground">
														{t("created_by")} #{team.created_by}
													</span>
												</div>
											</TableCell>
											<TableCell>
												<div className={TEAM_BADGE_CELL_CONTENT_CLASS}>
													<span className="text-sm font-medium text-foreground">
														{team.member_count}
													</span>
												</div>
											</TableCell>
											<TableCell>
												<TeamStorageCell
													team={team}
													policyGroupName={policyGroupNameById(
														team.policy_group_id,
													)}
												/>
											</TableCell>
											<TableCell>
												<div className={TEAM_TEXT_CELL_CONTENT_CLASS}>
													<span className="text-sm text-muted-foreground">
														{formatDateShort(
															team.archived_at ?? team.created_at,
														)}
													</span>
												</div>
											</TableCell>
											<TableCell
												onClick={(event) => event.stopPropagation()}
												onKeyDown={(event) => event.stopPropagation()}
											>
												<div className="flex justify-end">
													<Button
														variant="ghost"
														size="icon"
														className={ADMIN_ICON_BUTTON_CLASS}
														onClick={() =>
															navigate(`/admin/teams/${team.id}/overview`, {
																viewTransition: true,
															})
														}
														title={t("view_details")}
														aria-label={t("view_details")}
													>
														<Icon name="CaretRight" className="h-3.5 w-3.5" />
													</Button>
												</div>
											</TableCell>
										</TableRow>
									))}
								</TableBody>
							</Table>
						</ScrollArea>
					</AdminSurface>
				)}

				<AdminOffsetPagination
					total={total}
					currentPage={currentPage}
					totalPages={totalPages}
					pageSize={String(pageSize)}
					pageSizeOptions={pageSizeOptions}
					onPageSizeChange={handlePageSizeChange}
					prevDisabled={prevPageDisabled}
					nextDisabled={nextPageDisabled}
					onPrevious={() => setOffset(Math.max(0, offset - pageSize))}
					onNext={() => setOffset(offset + pageSize)}
				/>
			</AdminPageShell>
			<CreateTeamDialog
				open={createDialogOpen}
				form={createForm}
				submitting={submitting}
				policyGroupsLoading={policyGroupsLoading}
				policyGroupOptions={createPolicyGroupOptions}
				policyGroupUnavailable={createPolicyGroupUnavailable}
				onOpenChange={setCreateDialogOpen}
				onSubmit={(event) => void handleCreate(event)}
				onFieldChange={(key, value) =>
					setCreateForm((prev) => ({ ...prev, [key]: value }))
				}
			/>
		</AdminLayout>
	);
}
