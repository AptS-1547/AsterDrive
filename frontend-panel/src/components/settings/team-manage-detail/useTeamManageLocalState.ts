import { useReducer, useState } from "react";
import type { TeamMemberRole, UserStatus } from "@/types/api";

interface TeamDraft {
	baseDescription: string;
	baseName: string;
	description: string;
	name: string;
	teamId: number | null;
}

interface MemberState {
	identifier: string;
	offset: number;
	query: string;
	role: TeamMemberRole;
	roleFilter: "__all__" | TeamMemberRole;
	statusFilter: "__all__" | UserStatus;
	teamId: number | null;
}

interface TeamManageLocalState {
	archiveConfirmDraft: {
		teamId: number | null;
		value: string;
	};
	auditPageState: {
		offset: number;
		teamId: number | null;
	};
	memberState: MemberState;
	teamDraft: TeamDraft | null;
}

type TeamManageLocalAction =
	| { type: "archiveConfirmChanged"; teamId: number | null; value: string }
	| { type: "auditOffsetChanged"; offset: number; teamId: number | null }
	| {
			type: "memberChanged";
			patch: Partial<Omit<MemberState, "teamId">>;
			teamId: number | null;
	  }
	| { type: "teamDraftChanged"; draft: TeamDraft };

const defaultMemberState: Omit<MemberState, "teamId"> = {
	identifier: "",
	offset: 0,
	query: "",
	role: "member",
	roleFilter: "__all__",
	statusFilter: "__all__",
};

const initialTeamManageLocalState: TeamManageLocalState = {
	archiveConfirmDraft: {
		teamId: null,
		value: "",
	},
	auditPageState: {
		offset: 0,
		teamId: null,
	},
	memberState: {
		...defaultMemberState,
		teamId: null,
	},
	teamDraft: null,
};

function teamManageLocalReducer(
	state: TeamManageLocalState,
	action: TeamManageLocalAction,
): TeamManageLocalState {
	switch (action.type) {
		case "archiveConfirmChanged":
			return {
				...state,
				archiveConfirmDraft: {
					teamId: action.teamId,
					value: action.value,
				},
			};
		case "auditOffsetChanged":
			return {
				...state,
				auditPageState: {
					offset: action.offset,
					teamId: action.teamId,
				},
			};
		case "memberChanged": {
			const current =
				state.memberState.teamId === action.teamId
					? state.memberState
					: { ...defaultMemberState, teamId: action.teamId };
			return {
				...state,
				memberState: {
					...current,
					...action.patch,
					teamId: action.teamId,
				},
			};
		}
		case "teamDraftChanged":
			return {
				...state,
				teamDraft: action.draft,
			};
	}
}

export function useTeamManageLocalState(activeTeamId: number | null) {
	const [state, dispatch] = useReducer(
		teamManageLocalReducer,
		initialTeamManageLocalState,
	);
	const [webdavPrefix, setWebdavPrefix] = useState("/webdav");
	const memberState =
		state.memberState.teamId === activeTeamId
			? state.memberState
			: { ...defaultMemberState, teamId: activeTeamId };
	const auditOffset =
		state.auditPageState.teamId === activeTeamId
			? state.auditPageState.offset
			: 0;
	const archiveConfirmValue =
		state.archiveConfirmDraft.teamId === activeTeamId
			? state.archiveConfirmDraft.value
			: "";

	return {
		archiveConfirmValue,
		auditOffset,
		memberIdentifier: memberState.identifier,
		memberOffset: memberState.offset,
		memberQuery: memberState.query,
		memberRole: memberState.role,
		memberRoleFilter: memberState.roleFilter,
		memberStatusFilter: memberState.statusFilter,
		setArchiveConfirmValue: (value: string) =>
			dispatch({
				type: "archiveConfirmChanged",
				teamId: activeTeamId,
				value,
			}),
		setAuditOffset: (offset: number) =>
			dispatch({
				type: "auditOffsetChanged",
				offset,
				teamId: activeTeamId,
			}),
		setMemberIdentifier: (identifier: string) =>
			dispatch({
				type: "memberChanged",
				patch: { identifier },
				teamId: activeTeamId,
			}),
		setMemberOffset: (offset: number) =>
			dispatch({
				type: "memberChanged",
				patch: { offset },
				teamId: activeTeamId,
			}),
		setMemberQuery: (query: string) =>
			dispatch({
				type: "memberChanged",
				patch: { offset: 0, query },
				teamId: activeTeamId,
			}),
		setMemberRole: (role: TeamMemberRole) =>
			dispatch({
				type: "memberChanged",
				patch: { role },
				teamId: activeTeamId,
			}),
		setMemberRoleFilter: (roleFilter: "__all__" | TeamMemberRole) =>
			dispatch({
				type: "memberChanged",
				patch: { offset: 0, roleFilter },
				teamId: activeTeamId,
			}),
		setMemberStatusFilter: (statusFilter: "__all__" | UserStatus) =>
			dispatch({
				type: "memberChanged",
				patch: { offset: 0, statusFilter },
				teamId: activeTeamId,
			}),
		setTeamDraft: (draft: TeamDraft) =>
			dispatch({
				type: "teamDraftChanged",
				draft,
			}),
		setWebdavPrefix,
		teamDraft: state.teamDraft,
		webdavPrefix,
	};
}
