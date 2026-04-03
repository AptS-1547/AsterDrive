import { withQuery } from "@/lib/queryParams";
import { api } from "@/services/http";
import type {
	AddTeamMemberRequest,
	CreateTeamRequest,
	TeamInfo,
	TeamMemberInfo,
	UpdateTeamMemberRequest,
	UpdateTeamRequest,
} from "@/types/api";

export const teamService = {
	list: (params?: { archived?: boolean }) =>
		api.get<TeamInfo[]>(
			withQuery("/teams", {
				archived: params?.archived,
			}),
		),
	get: (id: number) => api.get<TeamInfo>(`/teams/${id}`),
	create: (data: CreateTeamRequest) => api.post<TeamInfo>("/teams", data),
	update: (id: number, data: UpdateTeamRequest) =>
		api.patch<TeamInfo>(`/teams/${id}`, data),
	delete: (id: number) => api.delete<void>(`/teams/${id}`),
	restore: (id: number) => api.post<TeamInfo>(`/teams/${id}/restore`),
	listMembers: (id: number) =>
		api.get<TeamMemberInfo[]>(`/teams/${id}/members`),
	addMember: (id: number, data: AddTeamMemberRequest) =>
		api.post<TeamMemberInfo>(`/teams/${id}/members`, data),
	updateMember: (
		id: number,
		memberUserId: number,
		data: UpdateTeamMemberRequest,
	) => api.patch<TeamMemberInfo>(`/teams/${id}/members/${memberUserId}`, data),
	removeMember: (id: number, memberUserId: number) =>
		api.delete<void>(`/teams/${id}/members/${memberUserId}`),
};
