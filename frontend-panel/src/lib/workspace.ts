export interface PersonalWorkspace {
	kind: "personal";
}

export interface TeamWorkspace {
	kind: "team";
	teamId: number;
}

export type Workspace = PersonalWorkspace | TeamWorkspace;

export const PERSONAL_WORKSPACE: PersonalWorkspace = { kind: "personal" };

export function isTeamWorkspace(
	workspace: Workspace,
): workspace is TeamWorkspace {
	return workspace.kind === "team";
}

export function workspaceEquals(a: Workspace, b: Workspace) {
	if (a.kind !== b.kind) return false;
	if (a.kind === "team" && b.kind === "team") {
		return a.teamId === b.teamId;
	}
	return true;
}

export function workspaceKey(workspace: Workspace) {
	return isTeamWorkspace(workspace) ? `team:${workspace.teamId}` : "personal";
}

export function workspaceApiPrefix(workspace: Workspace) {
	return isTeamWorkspace(workspace) ? `/teams/${workspace.teamId}` : "";
}

export function workspaceRootPath(workspace: Workspace) {
	return isTeamWorkspace(workspace) ? `/teams/${workspace.teamId}` : "/";
}

export function workspaceFolderPath(
	workspace: Workspace,
	folderId: number | null,
	folderName?: string,
) {
	if (folderId === null) return workspaceRootPath(workspace);

	const basePath = isTeamWorkspace(workspace)
		? `/teams/${workspace.teamId}/folder/${folderId}`
		: `/folder/${folderId}`;

	if (!folderName) return basePath;
	return `${basePath}?name=${encodeURIComponent(folderName)}`;
}

export function workspaceSharesPath(workspace: Workspace) {
	return isTeamWorkspace(workspace)
		? `/teams/${workspace.teamId}/shares`
		: "/shares";
}

export function workspaceTrashPath(workspace: Workspace) {
	return isTeamWorkspace(workspace)
		? `/teams/${workspace.teamId}/trash`
		: "/trash";
}

export function workspaceWebdavPath() {
	return "/settings/webdav";
}
