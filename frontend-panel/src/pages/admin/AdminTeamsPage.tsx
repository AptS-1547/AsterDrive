import {
	type FormEvent,
	useCallback,
	useDeferredValue,
	useEffect,
	useState,
} from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { AdminTeamDetailDialog } from "@/components/admin/AdminTeamDetailDialog";
import { AdminTableList } from "@/components/common/AdminTableList";
import { AdminLayout } from "@/components/layout/AdminLayout";
import { AdminPageHeader } from "@/components/layout/AdminPageHeader";
import { AdminPageShell } from "@/components/layout/AdminPageShell";
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
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import {
	TableCell,
	TableHead,
	TableHeader,
	TableRow,
} from "@/components/ui/table";
import { handleApiError } from "@/hooks/useApiError";
import { useApiList } from "@/hooks/useApiList";
import {
	ADMIN_CONTROL_HEIGHT_CLASS,
	ADMIN_ICON_BUTTON_CLASS,
	ADMIN_TABLE_ACTIONS_WIDTH_CLASS,
} from "@/lib/constants";
import { formatBytes, formatDateShort } from "@/lib/format";
import {
	adminPolicyGroupService,
	adminTeamService,
} from "@/services/adminService";
import type { AdminTeamInfo, StoragePolicyGroup } from "@/types/api";

const POLICY_GROUP_PAGE_SIZE = 100;

interface CreateTeamFormState {
	name: string;
	description: string;
	adminIdentifier: string;
	policyGroupId: string;
}

interface PolicyGroupOption {
	disabled?: boolean;
	label: string;
	value: string;
}

const EMPTY_CREATE_FORM: CreateTeamFormState = {
	name: "",
	description: "",
	adminIdentifier: "",
	policyGroupId: "",
};

const INTERACTIVE_TABLE_ROW_CLASS =
	"cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/50";

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
): PolicyGroupOption[] {
	const options: PolicyGroupOption[] = policyGroups
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

function TeamStorageBadge({
	team,
	policyGroupName,
}: {
	team: AdminTeamInfo;
	policyGroupName: string | null;
}) {
	const { t } = useTranslation(["admin", "core"]);

	return (
		<div className="flex flex-col gap-1">
			<span className="text-sm font-medium">
				{formatBytes(team.storage_used)}
				{team.storage_quota > 0
					? ` / ${formatBytes(team.storage_quota)}`
					: ` / ${t("core:unlimited")}`}
			</span>
			<span className="text-xs text-muted-foreground">
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
	const [keyword, setKeyword] = useState("");
	const [showArchived, setShowArchived] = useState(false);
	const deferredKeyword = useDeferredValue(keyword.trim());
	const [createDialogOpen, setCreateDialogOpen] = useState(false);
	const [detailTeamId, setDetailTeamId] = useState<number | null>(null);
	const [createForm, setCreateForm] =
		useState<CreateTeamFormState>(EMPTY_CREATE_FORM);
	const [submitting, setSubmitting] = useState(false);
	const [policyGroups, setPolicyGroups] = useState<StoragePolicyGroup[]>([]);
	const [policyGroupsLoading, setPolicyGroupsLoading] = useState(true);
	const {
		items: teams,
		loading,
		reload,
	} = useApiList(
		() =>
			adminTeamService.list({
				limit: 100,
				offset: 0,
				keyword: deferredKeyword || undefined,
				archived: showArchived,
			}),
		[deferredKeyword, showArchived],
	);

	const loadPolicyGroups = useCallback(async () => {
		setPolicyGroupsLoading(true);
		try {
			setPolicyGroups(
				await adminPolicyGroupService.listAll(POLICY_GROUP_PAGE_SIZE),
			);
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

	return (
		<AdminLayout>
			<AdminPageShell>
				<AdminPageHeader
					title={t("teams")}
					description={t("teams_intro")}
					actions={
						<Button onClick={handleOpenCreateDialog}>
							<Icon name="Plus" className="h-4 w-4" />
							{t("new_team")}
						</Button>
					}
					toolbar={
						<>
							<Input
								value={keyword}
								onChange={(event) => setKeyword(event.target.value)}
								placeholder={t("team_search_placeholder")}
								className={ADMIN_CONTROL_HEIGHT_CLASS}
							/>
							<Button
								variant={showArchived ? "default" : "outline"}
								className={ADMIN_CONTROL_HEIGHT_CLASS}
								onClick={() => setShowArchived((value) => !value)}
							>
								<Icon name="Cloud" className="h-4 w-4" />
								{showArchived
									? t("show_active_teams")
									: t("show_archived_teams")}
							</Button>
							<Button
								variant="outline"
								className={ADMIN_CONTROL_HEIGHT_CLASS}
								onClick={() => void reload()}
							>
								<Icon name="ArrowClockwise" className="h-4 w-4" />
								{t("refresh")}
							</Button>
						</>
					}
				/>
				<AdminTableList
					loading={loading}
					items={teams}
					columns={6}
					rows={6}
					emptyIcon={<Icon name="Cloud" className="h-10 w-10" />}
					emptyTitle={t("no_teams")}
					emptyDescription={t("no_teams_desc")}
					headerRow={
						<TableHeader>
							<TableRow>
								<TableHead>{t("core:name")}</TableHead>
								<TableHead>{t("created_by")}</TableHead>
								<TableHead>{t("member_count")}</TableHead>
								<TableHead>{t("quota")}</TableHead>
								<TableHead>{t("core:created_at")}</TableHead>
								<TableHead className={ADMIN_TABLE_ACTIONS_WIDTH_CLASS}>
									{t("core:actions")}
								</TableHead>
							</TableRow>
						</TableHeader>
					}
					renderRow={(team) => (
						<TableRow
							key={team.id}
							className={INTERACTIVE_TABLE_ROW_CLASS}
							onClick={() => setDetailTeamId(team.id)}
							onKeyDown={(event) => {
								if (event.key === "Enter" || event.key === " ") {
									event.preventDefault();
									setDetailTeamId(team.id);
								}
							}}
							tabIndex={0}
						>
							<TableCell>
								<div className="space-y-1">
									<div className="flex items-center gap-2">
										<span className="font-medium">{team.name}</span>
										<Badge variant="outline">#{team.id}</Badge>
										{team.archived_at ? (
											<Badge variant="outline">{t("archived_badge")}</Badge>
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
								<div className="space-y-1">
									<p className="text-sm">{team.created_by_username}</p>
									<p className="text-xs text-muted-foreground">
										{t("created_by")} #{team.created_by}
									</p>
								</div>
							</TableCell>
							<TableCell>{team.member_count}</TableCell>
							<TableCell>
								<TeamStorageBadge
									team={team}
									policyGroupName={policyGroupNameById(team.policy_group_id)}
								/>
							</TableCell>
							<TableCell className="text-muted-foreground text-sm">
								{formatDateShort(team.archived_at ?? team.created_at)}
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
										onClick={() => setDetailTeamId(team.id)}
										title={t("view_details")}
										aria-label={t("view_details")}
									>
										<Icon name="CaretRight" className="h-3.5 w-3.5" />
									</Button>
								</div>
							</TableCell>
						</TableRow>
					)}
				/>
			</AdminPageShell>

			<Dialog open={createDialogOpen} onOpenChange={setCreateDialogOpen}>
				<DialogContent>
					<form onSubmit={(event) => void handleCreate(event)}>
						<DialogHeader>
							<DialogTitle>{t("new_team")}</DialogTitle>
							<DialogDescription>{t("create_team_desc")}</DialogDescription>
						</DialogHeader>
						<div className="space-y-4 py-2">
							<div className="space-y-2">
								<Label htmlFor="admin-team-name">{t("core:name")}</Label>
								<Input
									id="admin-team-name"
									value={createForm.name}
									maxLength={128}
									disabled={submitting}
									onChange={(event) =>
										setCreateForm((prev) => ({
											...prev,
											name: event.target.value,
										}))
									}
								/>
							</div>
							<div className="space-y-2">
								<Label htmlFor="admin-team-admin">
									{t("team_admin_identifier")}
								</Label>
								<Input
									id="admin-team-admin"
									value={createForm.adminIdentifier}
									disabled={submitting}
									placeholder={t("team_admin_placeholder")}
									onChange={(event) =>
										setCreateForm((prev) => ({
											...prev,
											adminIdentifier: event.target.value,
										}))
									}
								/>
								<p className="text-xs text-muted-foreground">
									{t("team_admin_identifier_desc")}
								</p>
							</div>
							<div className="space-y-2">
								<Label>{t("team_policy_group")}</Label>
								<Select
									items={createPolicyGroupOptions}
									value={createForm.policyGroupId}
									onValueChange={(value) =>
										setCreateForm((prev) => ({
											...prev,
											policyGroupId: value ?? "",
										}))
									}
								>
									<SelectTrigger
										className="w-full"
										disabled={submitting || policyGroupsLoading}
									>
										<SelectValue placeholder={t("select_policy_group")} />
									</SelectTrigger>
									<SelectContent>
										{createPolicyGroupOptions.map((option) => (
											<SelectItem
												key={option.value}
												value={option.value}
												disabled={option.disabled}
											>
												{option.label}
											</SelectItem>
										))}
									</SelectContent>
								</Select>
								<p className="text-xs text-muted-foreground">
									{t("team_policy_group_desc")}
								</p>
								{createPolicyGroupUnavailable ? (
									<p className="text-xs text-destructive">
										{t("policy_group_no_assignable_groups")}
									</p>
								) : null}
							</div>
							<div className="space-y-2">
								<Label htmlFor="admin-team-description">
									{t("description")}
								</Label>
								<textarea
									id="admin-team-description"
									value={createForm.description}
									disabled={submitting}
									rows={4}
									className="min-h-24 w-full rounded-lg border border-input bg-transparent px-3 py-2 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50"
									onChange={(event) =>
										setCreateForm((prev) => ({
											...prev,
											description: event.target.value,
										}))
									}
								/>
							</div>
						</div>
						<DialogFooter>
							<Button
								type="submit"
								disabled={
									submitting ||
									!createForm.name.trim() ||
									!createForm.adminIdentifier.trim() ||
									!createForm.policyGroupId
								}
							>
								{t("create_team")}
							</Button>
						</DialogFooter>
					</form>
				</DialogContent>
			</Dialog>

			<AdminTeamDetailDialog
				open={detailTeamId !== null}
				teamId={detailTeamId}
				policyGroups={policyGroups}
				policyGroupsLoading={policyGroupsLoading}
				onListChange={reload}
				onOpenChange={(open) => {
					if (!open) {
						setDetailTeamId(null);
					}
				}}
				onRefreshPolicyGroups={loadPolicyGroups}
			/>
		</AdminLayout>
	);
}
