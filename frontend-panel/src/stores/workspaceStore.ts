import { create } from "zustand";
import {
	PERSONAL_WORKSPACE,
	type Workspace,
	workspaceEquals,
} from "@/lib/workspace";

interface WorkspaceState {
	workspace: Workspace;
	setWorkspace: (workspace: Workspace) => void;
}

export const useWorkspaceStore = create<WorkspaceState>((set) => ({
	workspace: PERSONAL_WORKSPACE,
	setWorkspace: (workspace) =>
		set((state) =>
			workspaceEquals(state.workspace, workspace) ? {} : { workspace },
		),
}));

export function getCurrentWorkspace() {
	return useWorkspaceStore.getState().workspace;
}

export function bindWorkspaceService<T extends object>(
	factory: (workspace: Workspace) => T,
): T {
	let cachedService: T | null = null;
	let cachedWorkspace: Workspace | null = null;

	return new Proxy({} as T, {
		get(_target, prop) {
			const workspace = getCurrentWorkspace();
			if (
				cachedService === null ||
				cachedWorkspace === null ||
				!workspaceEquals(cachedWorkspace, workspace)
			) {
				cachedWorkspace = workspace;
				cachedService = factory(workspace);
			}
			const service = cachedService;
			const value = service[prop as keyof T];
			return typeof value === "function" ? value.bind(service) : value;
		},
	});
}
