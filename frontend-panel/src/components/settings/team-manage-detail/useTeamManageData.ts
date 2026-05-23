import { useCallback, useEffect, useRef, useState } from "react";
import { handleApiError } from "@/hooks/useApiError";
import { isTeamManager, isTeamOwner } from "@/lib/team";
import { teamService } from "@/services/teamService";
import type {
	TeamAuditEntryInfo,
	TeamInfo,
	TeamMemberInfo,
	TeamMemberRole,
	UserStatus,
} from "@/types/api";
import {
	TEAM_MANAGE_AUDIT_PAGE_SIZE,
	TEAM_MANAGE_MEMBER_PAGE_SIZE,
} from "./teamManageDialogState";

interface TeamManageMemberFilters {
	keyword?: string;
	role?: TeamMemberRole;
	status?: UserStatus;
}

interface UseTeamManageDataArgs {
	auditOffset: number;
	memberFilters: TeamManageMemberFilters;
	memberOffset: number;
	open: boolean;
	teamId: number | null;
	teamSummary: TeamInfo | null;
}

export function useTeamManageData({
	auditOffset,
	memberFilters,
	memberOffset,
	open,
	teamId,
	teamSummary,
}: UseTeamManageDataArgs) {
	const [auditEntries, setAuditEntries] = useState<TeamAuditEntryInfo[]>([]);
	const [auditLoading, setAuditLoading] = useState(false);
	const [auditTotal, setAuditTotal] = useState(0);
	const [detailLoading, setDetailLoading] = useState(false);
	const [memberLoading, setMemberLoading] = useState(false);
	const [memberTotal, setMemberTotal] = useState(0);
	const [members, setMembers] = useState<TeamMemberInfo[]>([]);
	const [managerCount, setManagerCount] = useState(0);
	const [ownerCount, setOwnerCount] = useState(0);
	const [teamDetail, setTeamDetail] = useState<TeamInfo | null>(null);
	const auditRequestIdRef = useRef(0);
	const detailRequestIdRef = useRef(0);
	const memberRequestIdRef = useRef(0);
	const displayTeam = teamDetail ?? teamSummary;
	const viewerRole = displayTeam?.my_role ?? null;
	const canManageTeam = isTeamManager(viewerRole);
	const canAssignOwner = isTeamOwner(viewerRole);
	const canArchiveTeam = isTeamOwner(viewerRole);
	const detailRequestStarted = detailRequestIdRef.current > 0;

	const loadTeamDetail = useCallback(async (nextTeamId: number) => {
		const requestId = ++detailRequestIdRef.current;
		setDetailLoading(true);
		try {
			const detail = await teamService.get(nextTeamId);
			if (requestId !== detailRequestIdRef.current) {
				return;
			}

			setTeamDetail(detail);
		} catch (error) {
			if (requestId !== detailRequestIdRef.current) {
				return;
			}

			setTeamDetail(null);
			handleApiError(error);
		} finally {
			if (requestId === detailRequestIdRef.current) {
				setDetailLoading(false);
			}
		}
	}, []);

	const loadMembers = useCallback(
		async (
			nextTeamId: number,
			nextOffset = memberOffset,
			nextFilters: TeamManageMemberFilters = memberFilters,
		) => {
			const requestId = ++memberRequestIdRef.current;
			setMemberLoading(true);
			try {
				const page = await teamService.listMembers(nextTeamId, {
					keyword: nextFilters.keyword,
					role: nextFilters.role,
					status: nextFilters.status,
					limit: TEAM_MANAGE_MEMBER_PAGE_SIZE,
					offset: nextOffset,
				});
				if (requestId !== memberRequestIdRef.current) {
					return;
				}

				setMembers(page.items);
				setMemberTotal(page.total);
				setOwnerCount(page.owner_count);
				setManagerCount(page.manager_count);
			} catch (error) {
				if (requestId !== memberRequestIdRef.current) {
					return;
				}

				setMembers([]);
				setMemberTotal(0);
				setOwnerCount(0);
				setManagerCount(0);
				handleApiError(error);
			} finally {
				if (requestId === memberRequestIdRef.current) {
					setMemberLoading(false);
				}
			}
		},
		[memberFilters, memberOffset],
	);

	const loadAuditEntries = useCallback(
		async (nextTeamId: number, nextOffset = auditOffset) => {
			const requestId = ++auditRequestIdRef.current;
			setAuditLoading(true);
			try {
				const page = await teamService.listAuditLogs(nextTeamId, {
					limit: TEAM_MANAGE_AUDIT_PAGE_SIZE,
					offset: nextOffset,
				});
				if (requestId !== auditRequestIdRef.current) {
					return;
				}

				setAuditEntries(page.items);
				setAuditTotal(page.total);
			} catch (error) {
				if (requestId !== auditRequestIdRef.current) {
					return;
				}

				setAuditEntries([]);
				setAuditTotal(0);
				handleApiError(error);
			} finally {
				if (requestId === auditRequestIdRef.current) {
					setAuditLoading(false);
				}
			}
		},
		[auditOffset],
	);

	useEffect(() => {
		if (!open || teamId == null) {
			detailRequestIdRef.current += 1;
			setDetailLoading(false);
			setTeamDetail(null);
			return;
		}

		void loadTeamDetail(teamId);
	}, [loadTeamDetail, open, teamId]);

	useEffect(() => {
		if (!open || teamId == null || !canManageTeam) {
			auditRequestIdRef.current += 1;
			setAuditEntries([]);
			setAuditTotal(0);
			setAuditLoading(false);
			return;
		}

		void loadAuditEntries(teamId, auditOffset);
	}, [auditOffset, canManageTeam, loadAuditEntries, open, teamId]);

	useEffect(() => {
		if (!open || teamId == null) {
			memberRequestIdRef.current += 1;
			setMemberLoading(false);
			setMemberTotal(0);
			setMembers([]);
			setManagerCount(0);
			setOwnerCount(0);
			return;
		}

		void loadMembers(teamId, memberOffset, memberFilters);
	}, [loadMembers, memberFilters, memberOffset, open, teamId]);

	return {
		auditEntries,
		auditLoading,
		auditTotal,
		canArchiveTeam,
		canAssignOwner,
		canManageTeam,
		detailLoading,
		detailRequestStarted,
		displayTeam,
		loadAuditEntries,
		loadMembers,
		loadTeamDetail,
		managerCount,
		memberLoading,
		memberTotal,
		members,
		ownerCount,
		teamDetail,
		viewerRole,
	};
}
