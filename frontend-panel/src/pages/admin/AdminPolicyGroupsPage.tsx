import { type UIEvent, useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useSearchParams } from "react-router-dom";
import { toast } from "sonner";
import { AdminTableList } from "@/components/common/AdminTableList";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
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
	SelectGroup,
	SelectItem,
	SelectLabel,
	SelectSeparator,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import {
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
import { useConfirmDialog } from "@/hooks/useConfirmDialog";
import {
	ADMIN_CONTROL_HEIGHT_CLASS,
	ADMIN_ICON_BUTTON_CLASS,
} from "@/lib/constants";
import { formatBytes, formatDateAbsolute } from "@/lib/format";
import {
	adminPolicyGroupService,
	adminPolicyService,
} from "@/services/adminService";
import type {
	PolicyGroupUserMigrationResult,
	StoragePolicy,
	StoragePolicyGroup,
	StoragePolicyGroupItem,
} from "@/types/api";

const POLICY_GROUP_PAGE_SIZE_OPTIONS = [10, 20, 50] as const;
const POLICY_GROUP_LOOKUP_PAGE_SIZE = 100;
const POLICY_LOOKUP_PAGE_SIZE = 100;
const BYTES_PER_MB = 1024 * 1024;
const POLICY_GROUP_ACTIONS_WIDTH_CLASS = "w-32";
const INTERACTIVE_TABLE_ROW_CLASS =
	"cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/50";
const GROUP_TEXT_CELL_CONTENT_CLASS =
	"flex min-w-0 items-center rounded-lg bg-muted/10 px-3 py-3 text-left transition-colors duration-200";
const GROUP_BADGE_CELL_CONTENT_CLASS =
	"flex flex-wrap items-center gap-2 rounded-lg bg-muted/20 px-3 py-3 text-left transition-colors duration-200";

let nextRuleKey = 0;

interface PolicyGroupRuleForm {
	key: string;
	policyId: string;
	priority: string;
	minFileSizeMb: string;
	maxFileSizeMb: string;
}

interface PolicyGroupFormData {
	name: string;
	description: string;
	isEnabled: boolean;
	isDefault: boolean;
	items: PolicyGroupRuleForm[];
}

function createRuleKey() {
	nextRuleKey += 1;
	return `policy-group-rule-${nextRuleKey}`;
}

function bytesToMbInput(bytes: number) {
	if (bytes <= 0) return "";
	const mb = bytes / BYTES_PER_MB;
	return Number.isInteger(mb) ? String(mb) : String(Number(mb.toFixed(2)));
}

function mbInputToBytes(value: string) {
	if (!value.trim()) return 0;
	const parsed = Number(value);
	if (!Number.isFinite(parsed) || parsed <= 0) return 0;
	return Math.round(parsed * BYTES_PER_MB);
}

function buildRuleForm(
	policyId?: number | null,
	priority = 1,
	minFileSize = 0,
	maxFileSize = 0,
): PolicyGroupRuleForm {
	return {
		key: createRuleKey(),
		policyId: policyId != null ? String(policyId) : "",
		priority: String(priority),
		minFileSizeMb: bytesToMbInput(minFileSize),
		maxFileSizeMb: bytesToMbInput(maxFileSize),
	};
}

function getRuleRangeLabel(
	t: ReturnType<typeof useTranslation>["t"],
	item: Pick<StoragePolicyGroupItem, "min_file_size" | "max_file_size">,
) {
	if (item.min_file_size <= 0 && item.max_file_size <= 0) {
		return t("policy_group_range_any");
	}
	if (item.min_file_size > 0 && item.max_file_size <= 0) {
		return t("policy_group_range_min", {
			size: formatBytes(item.min_file_size),
		});
	}
	if (item.min_file_size <= 0 && item.max_file_size > 0) {
		return t("policy_group_range_max", {
			size: formatBytes(item.max_file_size),
		});
	}
	return t("policy_group_range_between", {
		min: formatBytes(item.min_file_size),
		max: formatBytes(item.max_file_size),
	});
}

function getMigrationSuccessMessage(
	t: ReturnType<typeof useTranslation>["t"],
	result: PolicyGroupUserMigrationResult,
	sourceName: string,
	targetName: string,
) {
	return t("policy_group_migration_success", {
		affected: result.affected_users,
		source: sourceName,
		target: targetName,
	});
}

function getDefaultGroupForm(policies: StoragePolicy[]): PolicyGroupFormData {
	return {
		name: "",
		description: "",
		isEnabled: true,
		isDefault: false,
		items: [buildRuleForm(policies[0]?.id ?? null)],
	};
}

function getGroupForm(group: StoragePolicyGroup): PolicyGroupFormData {
	return {
		name: group.name,
		description: group.description,
		isEnabled: group.is_enabled,
		isDefault: group.is_default,
		items: group.items.map((item) =>
			buildRuleForm(
				item.policy_id,
				item.priority,
				item.min_file_size,
				item.max_file_size,
			),
		),
	};
}

function mergePolicies(
	current: StoragePolicy[],
	incoming: StoragePolicy[],
): StoragePolicy[] {
	if (incoming.length === 0) return current;
	const merged = [...current];
	const seen = new Set(current.map((policy) => policy.id));
	for (const policy of incoming) {
		if (seen.has(policy.id)) continue;
		seen.add(policy.id);
		merged.push(policy);
	}
	return merged;
}

function matchesPolicySearch(policy: StoragePolicy, query: string) {
	if (!query) return true;
	const normalizedQuery = query.toLowerCase();
	return (
		policy.name.toLowerCase().includes(normalizedQuery) ||
		String(policy.id).includes(normalizedQuery) ||
		policy.driver_type.toLowerCase().includes(normalizedQuery)
	);
}

function findPolicyName(policies: StoragePolicy[], policyId: string) {
	if (!policyId) return null;
	return (
		policies.find((policy) => String(policy.id) === policyId)?.name ??
		`#${policyId}`
	);
}

export default function AdminPolicyGroupsPage() {
	const { t } = useTranslation("admin");
	const [searchParams, setSearchParams] = useSearchParams();
	const initialOffset = Number(searchParams.get("offset") ?? "0");
	const initialPageSize = Number(searchParams.get("pageSize") ?? "20");
	const [offset, setOffset] = useState(
		Number.isNaN(initialOffset) ? 0 : initialOffset,
	);
	const [pageSize, setPageSize] = useState<
		(typeof POLICY_GROUP_PAGE_SIZE_OPTIONS)[number]
	>(
		POLICY_GROUP_PAGE_SIZE_OPTIONS.includes(
			initialPageSize as (typeof POLICY_GROUP_PAGE_SIZE_OPTIONS)[number],
		)
			? (initialPageSize as (typeof POLICY_GROUP_PAGE_SIZE_OPTIONS)[number])
			: 20,
	);
	const {
		items: groups,
		total,
		loading,
		reload,
	} = useApiList(
		() => adminPolicyGroupService.list({ limit: pageSize, offset }),
		[offset, pageSize],
	);
	const [policies, setPolicies] = useState<StoragePolicy[]>([]);
	const [policiesTotal, setPoliciesTotal] = useState(0);
	const [policiesLoading, setPoliciesLoading] = useState(true);
	const [policiesLoadingMore, setPoliciesLoadingMore] = useState(false);
	const [policySearch, setPolicySearch] = useState("");
	const [dialogOpen, setDialogOpen] = useState(false);
	const [editingGroup, setEditingGroup] = useState<StoragePolicyGroup | null>(
		null,
	);
	const [migrationDialogOpen, setMigrationDialogOpen] = useState(false);
	const [migrationError, setMigrationError] = useState<string | null>(null);
	const [migrationSourceId, setMigrationSourceId] = useState<number | null>(
		null,
	);
	const [migrationSubmitting, setMigrationSubmitting] = useState(false);
	const [migrationTargetId, setMigrationTargetId] = useState("");
	const [migrationGroups, setMigrationGroups] = useState<
		StoragePolicyGroup[] | null
	>(null);
	const [migrationGroupsLoading, setMigrationGroupsLoading] = useState(false);
	const [form, setForm] = useState<PolicyGroupFormData>(() =>
		getDefaultGroupForm([]),
	);
	const [formError, setFormError] = useState<string | null>(null);
	const [submitting, setSubmitting] = useState(false);
	const totalPages = Math.max(1, Math.ceil(total / pageSize));
	const currentPage = Math.floor(offset / pageSize) + 1;
	const prevPageDisabled = offset === 0;
	const nextPageDisabled = offset + pageSize >= total;
	const hasMorePolicies = policies.length < policiesTotal;
	const refreshing = loading || policiesLoading || policiesLoadingMore;
	const pageSizeOptions = POLICY_GROUP_PAGE_SIZE_OPTIONS.map((size) => ({
		label: t("page_size_option", { count: size }),
		value: String(size),
	}));
	const normalizedPolicySearch = policySearch.trim().toLowerCase();
	const filteredPolicies = policies.filter((policy) =>
		matchesPolicySearch(policy, normalizedPolicySearch),
	);
	const availableMigrationGroups =
		migrationGroups ?? (total <= groups.length ? groups : []);
	const migrationSourceGroup =
		migrationSourceId === null
			? null
			: (availableMigrationGroups.find(
					(group) => group.id === migrationSourceId,
				) ??
				groups.find((group) => group.id === migrationSourceId) ??
				null);
	const migrationTargetOptions =
		migrationSourceGroup === null
			? []
			: availableMigrationGroups.filter(
					(group) => group.id !== migrationSourceGroup.id,
				);
	const migrationTargetSelectOptions = migrationTargetOptions.map((group) => ({
		label: group.name,
		value: String(group.id),
	}));
	const selectedMigrationTarget =
		migrationTargetOptions.find(
			(group) => String(group.id) === migrationTargetId,
		) ?? null;

	useEffect(() => {
		const params = new URLSearchParams();
		if (offset > 0) params.set("offset", String(offset));
		if (pageSize !== 20) params.set("pageSize", String(pageSize));
		setSearchParams(params, { replace: true });
	}, [offset, pageSize, setSearchParams]);

	const handlePageSizeChange = (value: string | null) => {
		if (!value) return;
		const next = Number(
			value,
		) as (typeof POLICY_GROUP_PAGE_SIZE_OPTIONS)[number];
		setPageSize(next);
		setOffset(0);
	};

	const loadPoliciesPage = useCallback(
		async (pageOffset: number, { reset = false }: { reset?: boolean } = {}) => {
			try {
				if (reset) {
					setPoliciesLoading(true);
				} else {
					setPoliciesLoadingMore(true);
				}

				const page = await adminPolicyService.list({
					limit: POLICY_LOOKUP_PAGE_SIZE,
					offset: pageOffset,
				});
				setPoliciesTotal(page.total);
				setPolicies((prev) =>
					reset ? page.items : mergePolicies(prev, page.items),
				);
			} catch (e) {
				handleApiError(e);
			} finally {
				if (reset) {
					setPoliciesLoading(false);
				} else {
					setPoliciesLoadingMore(false);
				}
			}
		},
		[],
	);

	useEffect(() => {
		void loadPoliciesPage(0, { reset: true });
	}, [loadPoliciesPage]);

	useEffect(() => {
		if (
			!migrationDialogOpen ||
			migrationSourceId === null ||
			migrationGroupsLoading
		) {
			return;
		}

		const nextTargets = availableMigrationGroups.filter(
			(group) => group.id !== migrationSourceId,
		);
		if (nextTargets.length === 0) {
			if (migrationTargetId) {
				setMigrationTargetId("");
			}
			return;
		}

		if (!nextTargets.some((group) => String(group.id) === migrationTargetId)) {
			setMigrationTargetId(String(nextTargets[0].id));
		}
	}, [
		availableMigrationGroups,
		migrationDialogOpen,
		migrationGroupsLoading,
		migrationSourceId,
		migrationTargetId,
	]);

	const reloadPolicies = useCallback(async () => {
		await loadPoliciesPage(0, { reset: true });
	}, [loadPoliciesPage]);

	const loadAllPolicyGroups = useCallback(async () => {
		try {
			setMigrationGroupsLoading(true);
			setMigrationGroups(
				await adminPolicyGroupService.listAll(POLICY_GROUP_LOOKUP_PAGE_SIZE),
			);
		} catch (e) {
			handleApiError(e);
		} finally {
			setMigrationGroupsLoading(false);
		}
	}, []);

	const loadMorePolicies = useCallback(async () => {
		if (policiesLoading || policiesLoadingMore || !hasMorePolicies) {
			return;
		}
		await loadPoliciesPage(policies.length);
	}, [
		hasMorePolicies,
		loadPoliciesPage,
		policies.length,
		policiesLoading,
		policiesLoadingMore,
	]);

	useEffect(() => {
		if (
			!dialogOpen ||
			!normalizedPolicySearch ||
			policiesLoading ||
			policiesLoadingMore ||
			!hasMorePolicies
		) {
			return;
		}

		if (filteredPolicies.length > 0) {
			return;
		}

		void loadMorePolicies();
	}, [
		dialogOpen,
		filteredPolicies.length,
		hasMorePolicies,
		loadMorePolicies,
		normalizedPolicySearch,
		policiesLoading,
		policiesLoadingMore,
	]);

	const handleRefresh = async () => {
		await Promise.all([reload(), reloadPolicies()]);
	};

	const handlePolicySelectOpenChange = (open: boolean) => {
		if (open && policies.length === 0 && !policiesLoading) {
			void reloadPolicies();
		}
	};

	const handlePolicySelectScroll = (event: UIEvent<HTMLDivElement>) => {
		if (policiesLoading || policiesLoadingMore || !hasMorePolicies) {
			return;
		}
		const target = event.currentTarget;
		if (target.scrollTop + target.clientHeight >= target.scrollHeight - 24) {
			void loadMorePolicies();
		}
	};

	const getSelectablePolicies = (selectedPolicyId: string) => {
		if (!selectedPolicyId) {
			return filteredPolicies;
		}

		const selectedPolicy = policies.find(
			(policy) => String(policy.id) === selectedPolicyId,
		);
		if (!selectedPolicy) {
			return filteredPolicies;
		}
		if (filteredPolicies.some((policy) => policy.id === selectedPolicy.id)) {
			return filteredPolicies;
		}
		return [selectedPolicy, ...filteredPolicies];
	};

	const setField = <K extends keyof PolicyGroupFormData>(
		key: K,
		value: PolicyGroupFormData[K],
	) => {
		setForm((prev) => ({ ...prev, [key]: value }));
		setFormError(null);
	};

	const setRuleField = <K extends Exclude<keyof PolicyGroupRuleForm, "key">>(
		ruleKey: string,
		key: K,
		value: PolicyGroupRuleForm[K],
	) => {
		setForm((prev) => ({
			...prev,
			items: prev.items.map((item) =>
				item.key === ruleKey ? { ...item, [key]: value } : item,
			),
		}));
		setFormError(null);
	};

	const getNextPolicyId = () => {
		const selected = new Set(
			form.items.map((item) => item.policyId).filter(Boolean),
		);
		return (
			policies.find((policy) => !selected.has(String(policy.id)))?.id ??
			policies[0]?.id ??
			null
		);
	};

	const addRule = () => {
		setForm((prev) => ({
			...prev,
			items: [
				...prev.items,
				buildRuleForm(getNextPolicyId(), prev.items.length + 1),
			],
		}));
		setFormError(null);
	};

	const removeRule = (ruleKey: string) => {
		setForm((prev) => ({
			...prev,
			items: prev.items.filter((item) => item.key !== ruleKey),
		}));
		setFormError(null);
	};

	const openCreate = () => {
		setEditingGroup(null);
		setForm(getDefaultGroupForm(policies));
		setFormError(null);
		setPolicySearch("");
		setDialogOpen(true);
	};

	const openEdit = (group: StoragePolicyGroup) => {
		setPolicies((prev) =>
			mergePolicies(
				prev,
				group.items.map((item) => item.policy),
			),
		);
		setEditingGroup(group);
		setForm(getGroupForm(group));
		setFormError(null);
		setPolicySearch("");
		setDialogOpen(true);
	};

	const openMigrationDialog = (group: StoragePolicyGroup) => {
		setMigrationSourceId(group.id);
		setMigrationTargetId("");
		setMigrationError(null);
		setMigrationGroups(total <= groups.length ? groups : null);
		setMigrationDialogOpen(true);
		if (total > groups.length) {
			void loadAllPolicyGroups();
		}
	};

	const handleDialogOpenChange = (open: boolean) => {
		setDialogOpen(open);
		if (!open) {
			setSubmitting(false);
			setFormError(null);
			setPolicySearch("");
		}
	};

	const handleMigrationDialogOpenChange = (open: boolean) => {
		setMigrationDialogOpen(open);
		if (!open) {
			setMigrationError(null);
			setMigrationGroups(null);
			setMigrationGroupsLoading(false);
			setMigrationSourceId(null);
			setMigrationSubmitting(false);
			setMigrationTargetId("");
		}
	};

	const validateForm = () => {
		if (!form.name.trim()) {
			return t("policy_group_name_required");
		}
		if (form.isDefault && !form.isEnabled) {
			return t("policy_group_default_requires_enabled");
		}
		if (policies.length === 0) {
			return t("policy_group_no_policies_available");
		}
		if (form.items.length === 0) {
			return t("policy_group_rule_required");
		}

		const seenPolicyIds = new Set<string>();
		const seenPriorities = new Set<number>();

		for (const item of form.items) {
			if (!item.policyId) {
				return t("policy_group_rule_policy_required");
			}
			const priority = Number(item.priority);
			if (!Number.isInteger(priority) || priority <= 0) {
				return t("policy_group_rule_priority_invalid");
			}
			if (seenPolicyIds.has(item.policyId)) {
				return t("policy_group_rule_policy_duplicate");
			}
			if (seenPriorities.has(priority)) {
				return t("policy_group_rule_priority_duplicate");
			}
			seenPolicyIds.add(item.policyId);
			seenPriorities.add(priority);

			const min = item.minFileSizeMb.trim() ? Number(item.minFileSizeMb) : 0;
			const max = item.maxFileSizeMb.trim() ? Number(item.maxFileSizeMb) : 0;
			if (
				!Number.isFinite(min) ||
				!Number.isFinite(max) ||
				min < 0 ||
				max < 0
			) {
				return t("policy_group_rule_size_invalid");
			}
			if (max > 0 && max <= min) {
				return t("policy_group_rule_range_invalid");
			}
		}

		return null;
	};

	const submitForm = async () => {
		const validationError = validateForm();
		if (validationError) {
			setFormError(validationError);
			return;
		}

		const payload = {
			name: form.name.trim(),
			description: form.description.trim() || undefined,
			is_enabled: form.isEnabled,
			is_default: form.isDefault,
			items: [...form.items]
				.map((item) => ({
					policy_id: Number(item.policyId),
					priority: Number(item.priority),
					min_file_size: mbInputToBytes(item.minFileSizeMb),
					max_file_size: mbInputToBytes(item.maxFileSizeMb),
				}))
				.sort((a, b) => a.priority - b.priority),
		};

		try {
			setSubmitting(true);
			if (editingGroup) {
				await adminPolicyGroupService.update(editingGroup.id, payload);
				await reload();
				toast.success(t("policy_group_updated"));
			} else {
				await adminPolicyGroupService.create(payload);
				const nextTotal = total + 1;
				const nextLastOffset = Math.max(
					0,
					Math.floor((nextTotal - 1) / pageSize) * pageSize,
				);
				if (nextLastOffset !== offset) {
					setOffset(nextLastOffset);
				} else {
					await reload();
				}
				toast.success(t("policy_group_created"));
			}
			handleDialogOpenChange(false);
		} catch (e) {
			handleApiError(e);
		} finally {
			setSubmitting(false);
		}
	};

	const handleDelete = async (id: number) => {
		try {
			await adminPolicyGroupService.delete(id);
			if (groups.length === 1 && offset > 0) {
				setOffset(Math.max(0, offset - pageSize));
			} else {
				await reload();
			}
			toast.success(t("policy_group_deleted"));
		} catch (e) {
			handleApiError(e);
		}
	};

	const handleMigrateUsers = async () => {
		if (!migrationSourceGroup) {
			return;
		}
		if (!migrationTargetId) {
			setMigrationError(t("policy_group_migration_target_required"));
			return;
		}

		const targetGroupId = Number(migrationTargetId);
		if (!Number.isInteger(targetGroupId)) {
			setMigrationError(t("policy_group_migration_target_required"));
			return;
		}
		if (targetGroupId === migrationSourceGroup.id) {
			setMigrationError(t("policy_group_migration_same_group_invalid"));
			return;
		}

		const targetGroupName =
			selectedMigrationTarget?.name ?? `#${targetGroupId}`;

		try {
			setMigrationSubmitting(true);
			setMigrationError(null);
			const result = await adminPolicyGroupService.migrateUsers(
				migrationSourceGroup.id,
				{ target_group_id: targetGroupId },
			);
			await reload();
			toast.success(
				getMigrationSuccessMessage(
					t,
					result,
					migrationSourceGroup.name,
					targetGroupName,
				),
			);
			handleMigrationDialogOpenChange(false);
		} catch (e) {
			handleApiError(e);
		} finally {
			setMigrationSubmitting(false);
		}
	};

	const {
		confirmId: deleteId,
		requestConfirm,
		dialogProps,
	} = useConfirmDialog(handleDelete);

	const deleteGroupName =
		deleteId !== null
			? (groups.find((group) => group.id === deleteId)?.name ?? "")
			: "";

	return (
		<AdminLayout>
			<AdminPageShell>
				<AdminPageHeader
					title={t("policy_groups")}
					description={t("policy_groups_intro")}
					actions={
						<>
							<Button
								size="sm"
								className={ADMIN_CONTROL_HEIGHT_CLASS}
								onClick={openCreate}
								disabled={policiesLoading || policies.length === 0}
							>
								<Icon name="Plus" className="mr-1 h-4 w-4" />
								{t("new_policy_group")}
							</Button>
							<Button
								variant="outline"
								size="sm"
								className={ADMIN_CONTROL_HEIGHT_CLASS}
								onClick={() => void handleRefresh()}
								disabled={refreshing}
							>
								<Icon
									name={refreshing ? "Spinner" : "ArrowsClockwise"}
									className={`mr-1 h-3.5 w-3.5 ${refreshing ? "animate-spin" : ""}`}
								/>
								{t("refresh")}
							</Button>
						</>
					}
				/>

				<AdminTableList
					loading={loading}
					items={groups}
					columns={6}
					rows={5}
					emptyIcon={<Icon name="ListBullets" className="h-6 w-6" />}
					emptyTitle={t("no_policy_groups")}
					emptyDescription={t("no_policy_groups_desc")}
					headerRow={
						<TableHeader>
							<TableRow>
								<TableHead className="w-16">{t("id")}</TableHead>
								<TableHead>{t("core:name")}</TableHead>
								<TableHead>{t("policy_group_rules")}</TableHead>
								<TableHead>{t("policy_group_status")}</TableHead>
								<TableHead>{t("core:updated_at")}</TableHead>
								<TableHead className={POLICY_GROUP_ACTIONS_WIDTH_CLASS}>
									{t("core:actions")}
								</TableHead>
							</TableRow>
						</TableHeader>
					}
					renderRow={(group) => (
						<TableRow
							key={group.id}
							className={INTERACTIVE_TABLE_ROW_CLASS}
							onClick={() => openEdit(group)}
							onKeyDown={(event) => {
								if (event.key === "Enter" || event.key === " ") {
									event.preventDefault();
									openEdit(group);
								}
							}}
							tabIndex={0}
						>
							<TableCell>
								<div className={GROUP_TEXT_CELL_CONTENT_CLASS}>
									<span className="font-mono text-xs text-muted-foreground">
										{group.id}
									</span>
								</div>
							</TableCell>
							<TableCell>
								<div className="flex min-w-0 flex-col gap-2 rounded-lg bg-muted/10 px-3 py-3 text-left">
									<div className="truncate font-medium text-foreground">
										{group.name}
									</div>
									{group.description ? (
										<p className="line-clamp-2 text-xs text-muted-foreground">
											{group.description}
										</p>
									) : (
										<span className="text-xs text-muted-foreground">
											{t("policy_group_description_empty")}
										</span>
									)}
								</div>
							</TableCell>
							<TableCell>
								<div className="flex min-w-0 flex-col gap-2 rounded-lg bg-muted/10 px-3 py-3 text-left">
									{group.items.slice(0, 2).map((item) => (
										<div
											key={item.id}
											className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground"
										>
											<Badge variant="outline">{item.policy.name}</Badge>
											<span>
												{t("policy_group_priority_short", {
													priority: item.priority,
												})}
											</span>
											<span>{getRuleRangeLabel(t, item)}</span>
										</div>
									))}
									{group.items.length > 2 ? (
										<span className="text-xs text-muted-foreground">
											{t("policy_group_more_rules", {
												count: group.items.length - 2,
											})}
										</span>
									) : null}
								</div>
							</TableCell>
							<TableCell>
								<div className={GROUP_BADGE_CELL_CONTENT_CLASS}>
									{group.is_default ? (
										<Badge className="border-blue-300 bg-blue-100 text-blue-700 dark:border-blue-700 dark:bg-blue-900 dark:text-blue-300">
											{t("is_default")}
										</Badge>
									) : null}
									<Badge
										variant="outline"
										className={
											group.is_enabled
												? "border-emerald-500/60 bg-emerald-500/10 text-emerald-600 dark:text-emerald-300"
												: "border-muted-foreground/30 bg-muted text-muted-foreground"
										}
									>
										{group.is_enabled
											? t("core:active")
											: t("core:disabled_status")}
									</Badge>
								</div>
							</TableCell>
							<TableCell>
								<div className={GROUP_TEXT_CELL_CONTENT_CLASS}>
									<span className="text-xs text-muted-foreground">
										{formatDateAbsolute(group.updated_at)}
									</span>
								</div>
							</TableCell>
							<TableCell
								onClick={(event) => event.stopPropagation()}
								onKeyDown={(event) => event.stopPropagation()}
							>
								<TooltipProvider>
									<div className="flex justify-end gap-1">
										<Tooltip>
											<TooltipTrigger>
												<div>
													<Button
														variant="ghost"
														size="icon"
														className={ADMIN_ICON_BUTTON_CLASS}
														onClick={() => openMigrationDialog(group)}
														aria-label={t("migrate_policy_group_users")}
														title={t("migrate_policy_group_users")}
														disabled={total <= 1}
													>
														<Icon
															name="ArrowsClockwise"
															className="h-3.5 w-3.5"
														/>
													</Button>
												</div>
											</TooltipTrigger>
											{total <= 1 ? (
												<TooltipContent>
													{t("policy_group_migration_unavailable")}
												</TooltipContent>
											) : null}
										</Tooltip>
										<Button
											variant="ghost"
											size="icon"
											className={ADMIN_ICON_BUTTON_CLASS}
											onClick={() => openEdit(group)}
											aria-label={t("edit_policy_group")}
											title={t("edit_policy_group")}
										>
											<Icon name="PencilSimple" className="h-3.5 w-3.5" />
										</Button>
										<Tooltip>
											<TooltipTrigger>
												<div>
													<Button
														variant="ghost"
														size="icon"
														className={`${ADMIN_ICON_BUTTON_CLASS} text-destructive`}
														onClick={() => requestConfirm(group.id)}
														aria-label={t("delete_policy_group")}
														title={t("delete_policy_group")}
														disabled={group.is_default}
													>
														<Icon name="Trash" className="h-3.5 w-3.5" />
													</Button>
												</div>
											</TooltipTrigger>
											{group.is_default ? (
												<TooltipContent>
													{t("policy_group_delete_default_blocked")}
												</TooltipContent>
											) : null}
										</Tooltip>
									</div>
								</TooltipProvider>
							</TableCell>
						</TableRow>
					)}
				/>

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

				<Dialog
					open={migrationDialogOpen}
					onOpenChange={handleMigrationDialogOpenChange}
				>
					<DialogContent className="sm:max-w-lg">
						<DialogHeader>
							<DialogTitle>{t("migrate_policy_group_users")}</DialogTitle>
							<DialogDescription>
								{t("policy_group_migration_desc")}
							</DialogDescription>
						</DialogHeader>
						<div className="space-y-4">
							<div className="grid gap-4 sm:grid-cols-2">
								<div className="space-y-2">
									<Label>{t("policy_group_migration_source")}</Label>
									<div className="rounded-lg border bg-muted/20 px-3 py-2 text-sm font-medium text-foreground">
										{migrationSourceGroup?.name ?? "-"}
									</div>
								</div>
								<div className="space-y-2">
									<Label>{t("policy_group_migration_target")}</Label>
									<Select
										items={migrationTargetSelectOptions}
										value={migrationTargetId}
										onValueChange={(value) => {
											setMigrationTargetId(value ?? "");
											setMigrationError(null);
										}}
										disabled={
											migrationGroupsLoading ||
											migrationSubmitting ||
											migrationTargetOptions.length === 0
										}
									>
										<SelectTrigger className={ADMIN_CONTROL_HEIGHT_CLASS}>
											<SelectValue
												placeholder={t(
													"policy_group_migration_target_placeholder",
												)}
											/>
										</SelectTrigger>
										<SelectContent>
											{migrationTargetOptions.map((group) => (
												<SelectItem key={group.id} value={String(group.id)}>
													{group.name}
												</SelectItem>
											))}
										</SelectContent>
									</Select>
								</div>
							</div>

							<div className="rounded-lg border bg-muted/20 px-3 py-2 text-sm text-muted-foreground">
								{t("policy_group_migration_hint")}
							</div>

							{migrationError ? (
								<div className="rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive">
									{migrationError}
								</div>
							) : null}
						</div>
						<DialogFooter className="gap-2">
							<Button
								type="button"
								variant="outline"
								onClick={() => handleMigrationDialogOpenChange(false)}
							>
								{t("core:cancel")}
							</Button>
							<Button
								type="button"
								onClick={() => void handleMigrateUsers()}
								disabled={
									migrationGroupsLoading ||
									migrationSubmitting ||
									migrationTargetOptions.length === 0
								}
							>
								{migrationSubmitting ? (
									<Icon name="Spinner" className="mr-1 h-4 w-4 animate-spin" />
								) : (
									<Icon name="ArrowsClockwise" className="mr-1 h-4 w-4" />
								)}
								{t("policy_group_migration_confirm")}
							</Button>
						</DialogFooter>
					</DialogContent>
				</Dialog>

				<ConfirmDialog
					{...dialogProps}
					title={`${t("delete_policy_group")} "${deleteGroupName}"?`}
					description={t("delete_policy_group_desc")}
					confirmLabel={t("core:delete")}
					variant="destructive"
				/>

				<Dialog open={dialogOpen} onOpenChange={handleDialogOpenChange}>
					<DialogContent className="flex max-h-[min(90vh,calc(100vh-2rem))] flex-col gap-0 overflow-hidden p-0 sm:max-w-[calc(100%-2rem)] lg:max-w-5xl">
						<form
							className="flex min-h-0 flex-1 flex-col overflow-hidden"
							autoComplete="off"
							onSubmit={(event) => {
								event.preventDefault();
								void submitForm();
							}}
						>
							<DialogHeader className="shrink-0 border-b px-6 pt-5 pb-0 pr-14">
								<DialogTitle>
									{editingGroup
										? t("edit_policy_group")
										: t("create_policy_group")}
								</DialogTitle>
								<DialogDescription>
									{t("policy_group_dialog_desc")}
								</DialogDescription>
							</DialogHeader>
							<div className="min-h-0 flex-1 overflow-y-auto px-6 pt-6 pb-5">
								<div className="grid gap-6 lg:grid-cols-[320px_minmax(0,1fr)]">
									<section className="space-y-4 rounded-xl border bg-muted/20 p-4 lg:sticky lg:top-0 lg:self-start">
										<div className="space-y-1">
											<h3 className="text-sm font-semibold text-foreground">
												{t("policy_group_overview")}
											</h3>
											<p className="text-xs text-muted-foreground">
												{t("policy_group_overview_desc")}
											</p>
										</div>

										<div className="space-y-2">
											<Label htmlFor="policy-group-name">
												{t("core:name")}
											</Label>
											<Input
												id="policy-group-name"
												value={form.name}
												onChange={(event) =>
													setField("name", event.target.value)
												}
												className={ADMIN_CONTROL_HEIGHT_CLASS}
												aria-invalid={!form.name.trim() ? true : undefined}
												required
											/>
										</div>

										<div className="space-y-2">
											<Label htmlFor="policy-group-description">
												{t("policy_group_description")}
											</Label>
											<Input
												id="policy-group-description"
												value={form.description}
												onChange={(event) =>
													setField("description", event.target.value)
												}
												className={ADMIN_CONTROL_HEIGHT_CLASS}
												placeholder={t("policy_group_description_placeholder")}
											/>
										</div>

										<div className="space-y-3 rounded-xl border bg-background p-4">
											<div className="flex items-center justify-between gap-3">
												<div className="space-y-1">
													<p className="text-sm font-medium text-foreground">
														{t("policy_group_enabled")}
													</p>
													<p className="text-xs text-muted-foreground">
														{t("policy_group_enabled_desc")}
													</p>
												</div>
												<Switch
													id="policy-group-enabled"
													checked={form.isEnabled}
													onCheckedChange={(checked) =>
														setField("isEnabled", checked)
													}
												/>
											</div>
											<div className="flex items-center justify-between gap-3">
												<div className="space-y-1">
													<p className="text-sm font-medium text-foreground">
														{t("policy_group_default")}
													</p>
													<p className="text-xs text-muted-foreground">
														{t("policy_group_default_desc")}
													</p>
												</div>
												<Switch
													id="policy-group-default"
													checked={form.isDefault}
													onCheckedChange={(checked) =>
														setField("isDefault", checked)
													}
												/>
											</div>
										</div>

										<div className="rounded-xl border bg-background p-4">
											<p className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
												{t("policy_group_summary")}
											</p>
											<div className="mt-3 flex flex-wrap gap-2">
												<Badge variant="outline">
													{t("policy_group_rules_count", {
														count: form.items.length,
													})}
												</Badge>
												{form.isDefault ? (
													<Badge className="border-blue-300 bg-blue-100 text-blue-700 dark:border-blue-700 dark:bg-blue-900 dark:text-blue-300">
														{t("is_default")}
													</Badge>
												) : null}
												<Badge
													variant="outline"
													className={
														form.isEnabled
															? "border-emerald-500/60 bg-emerald-500/10 text-emerald-600 dark:text-emerald-300"
															: "border-muted-foreground/30 bg-muted text-muted-foreground"
													}
												>
													{form.isEnabled
														? t("core:active")
														: t("core:disabled_status")}
												</Badge>
											</div>
										</div>
									</section>

									<section className="space-y-4 rounded-xl border bg-background p-4">
										<div className="space-y-3">
											<div className="flex items-start justify-between gap-4">
												<div className="space-y-1">
													<h3 className="text-sm font-semibold text-foreground">
														{t("policy_group_rules")}
													</h3>
													<p className="text-xs text-muted-foreground">
														{t("policy_group_rules_desc")}
													</p>
												</div>
												<Button
													type="button"
													variant="outline"
													size="sm"
													className={ADMIN_CONTROL_HEIGHT_CLASS}
													onClick={addRule}
													disabled={policies.length === 0}
												>
													<Icon name="Plus" className="mr-1 h-4 w-4" />
													{t("policy_group_add_rule")}
												</Button>
											</div>

											<div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_auto] md:items-start">
												<div className="space-y-2">
													<Label htmlFor="policy-group-search">
														{t("policy_group_policy_search")}
													</Label>
													<Input
														id="policy-group-search"
														value={policySearch}
														onChange={(event) =>
															setPolicySearch(event.target.value)
														}
														className={ADMIN_CONTROL_HEIGHT_CLASS}
														placeholder={t(
															"policy_group_policy_search_placeholder",
														)}
													/>
												</div>
												<div className="rounded-lg border bg-muted/20 px-3 py-2 text-xs text-muted-foreground">
													{policiesLoadingMore
														? t("policy_group_loading_more_policies")
														: hasMorePolicies
															? t("policy_group_scroll_to_load_more")
															: t("policy_group_all_policies_loaded", {
																	total: policiesTotal,
																})}
												</div>
											</div>
										</div>

										{policies.length === 0 ? (
											<div className="rounded-xl border border-dashed px-4 py-6 text-sm text-muted-foreground">
												{t("policy_group_no_policies_available")}
											</div>
										) : null}

										<div className="space-y-3">
											{form.items.map((item, index) => {
												const selectablePolicies = getSelectablePolicies(
													item.policyId,
												);
												const selectablePolicyOptions = selectablePolicies.map(
													(policy) => ({
														label: policy.name,
														value: String(policy.id),
													}),
												);
												const selectedPolicyName = findPolicyName(
													policies,
													item.policyId,
												);

												return (
													<div
														key={item.key}
														className="space-y-4 rounded-xl border bg-muted/20 p-4"
													>
														<div className="flex items-center justify-between gap-3">
															<div>
																<p className="text-sm font-medium text-foreground">
																	{t("policy_group_rule_title", {
																		index: index + 1,
																	})}
																</p>
																<p className="text-xs text-muted-foreground">
																	{t("policy_group_rule_hint")}
																</p>
															</div>
															<Button
																type="button"
																variant="ghost"
																size="icon"
																className={`${ADMIN_ICON_BUTTON_CLASS} text-muted-foreground`}
																onClick={() => removeRule(item.key)}
																disabled={form.items.length === 1}
																aria-label={t("policy_group_remove_rule")}
															>
																<Icon name="Trash" className="h-3.5 w-3.5" />
															</Button>
														</div>

														<div className="grid gap-4 md:grid-cols-[minmax(0,1.5fr)_120px]">
															<div className="space-y-2">
																<Label>{t("assign_policy")}</Label>
																<Select
																	items={selectablePolicyOptions}
																	value={item.policyId}
																	onOpenChange={handlePolicySelectOpenChange}
																	onValueChange={(value) =>
																		setRuleField(
																			item.key,
																			"policyId",
																			value ?? "",
																		)
																	}
																>
																	<SelectTrigger
																		className={ADMIN_CONTROL_HEIGHT_CLASS}
																	>
																		<SelectValue
																			placeholder={t("select_policy")}
																		>
																			{selectedPolicyName}
																		</SelectValue>
																	</SelectTrigger>
																	<SelectContent
																		className="max-h-64"
																		onScroll={handlePolicySelectScroll}
																	>
																		{selectablePolicies.map((policy) => (
																			<SelectItem
																				key={policy.id}
																				value={String(policy.id)}
																			>
																				{policy.name}
																			</SelectItem>
																		))}
																		{selectablePolicies.length === 0 ? (
																			<SelectGroup>
																				<SelectLabel>
																					{t(
																						"policy_group_no_filtered_policies",
																					)}
																				</SelectLabel>
																			</SelectGroup>
																		) : null}
																		{policiesLoadingMore || hasMorePolicies ? (
																			<>
																				{selectablePolicies.length > 0 ? (
																					<SelectSeparator />
																				) : null}
																				<SelectGroup>
																					<SelectLabel>
																						{policiesLoadingMore
																							? t(
																									"policy_group_loading_more_policies",
																								)
																							: t(
																									"policy_group_scroll_to_load_more",
																								)}
																					</SelectLabel>
																				</SelectGroup>
																			</>
																		) : null}
																	</SelectContent>
																</Select>
															</div>

															<div className="space-y-2">
																<Label htmlFor={`${item.key}-priority`}>
																	{t("policy_group_priority")}
																</Label>
																<Input
																	id={`${item.key}-priority`}
																	type="number"
																	min="1"
																	step="1"
																	value={item.priority}
																	onChange={(event) =>
																		setRuleField(
																			item.key,
																			"priority",
																			event.target.value,
																		)
																	}
																	className={ADMIN_CONTROL_HEIGHT_CLASS}
																/>
															</div>
														</div>

														<div className="grid gap-4 md:grid-cols-2">
															<div className="space-y-2">
																<Label htmlFor={`${item.key}-min-size`}>
																	{t("policy_group_min_size_mb")}
																</Label>
																<Input
																	id={`${item.key}-min-size`}
																	type="number"
																	min="0"
																	step="0.1"
																	value={item.minFileSizeMb}
																	onChange={(event) =>
																		setRuleField(
																			item.key,
																			"minFileSizeMb",
																			event.target.value,
																		)
																	}
																	placeholder={t("policy_group_size_unlimited")}
																	className={ADMIN_CONTROL_HEIGHT_CLASS}
																/>
															</div>
															<div className="space-y-2">
																<Label htmlFor={`${item.key}-max-size`}>
																	{t("policy_group_max_size_mb")}
																</Label>
																<Input
																	id={`${item.key}-max-size`}
																	type="number"
																	min="0"
																	step="0.1"
																	value={item.maxFileSizeMb}
																	onChange={(event) =>
																		setRuleField(
																			item.key,
																			"maxFileSizeMb",
																			event.target.value,
																		)
																	}
																	placeholder={t("policy_group_size_unlimited")}
																	className={ADMIN_CONTROL_HEIGHT_CLASS}
																/>
															</div>
														</div>
													</div>
												);
											})}
										</div>
									</section>
								</div>

								{formError ? (
									<div className="mt-4 rounded-xl border border-destructive/30 bg-destructive/5 px-4 py-3 text-sm text-destructive">
										{formError}
									</div>
								) : null}
							</div>
							<DialogFooter className="mx-0 mb-0 w-full shrink-0 flex-row items-center gap-2 rounded-b-xl px-6 py-3">
								<Button
									type="button"
									variant="outline"
									onClick={() => handleDialogOpenChange(false)}
								>
									{t("core:cancel")}
								</Button>
								<Button type="submit" disabled={submitting || policiesLoading}>
									{submitting ? (
										<Icon
											name="Spinner"
											className="mr-1 h-4 w-4 animate-spin"
										/>
									) : (
										<Icon name="FloppyDisk" className="mr-1 h-4 w-4" />
									)}
									{editingGroup ? t("save_changes") : t("core:create")}
								</Button>
							</DialogFooter>
						</form>
					</DialogContent>
				</Dialog>
			</AdminPageShell>
		</AdminLayout>
	);
}
