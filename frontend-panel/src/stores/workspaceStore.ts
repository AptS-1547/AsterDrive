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

	function getService(): T {
		const workspace = getCurrentWorkspace();
		if (
			cachedService === null ||
			cachedWorkspace === null ||
			!workspaceEquals(cachedWorkspace, workspace)
		) {
			cachedWorkspace = workspace;
			cachedService = factory(workspace);
		}
		return cachedService;
	}

	return new Proxy({} as T, {
		get(_target, prop) {
			const service = getService();
			const value = service[prop as keyof T];
			return typeof value === "function" ? value.bind(service) : value;
		},
		has(_target, prop) {
			return prop in getService();
		},
		ownKeys() {
			return Reflect.ownKeys(getService());
		},
		getOwnPropertyDescriptor(_target, prop) {
			const service = getService();
			const descriptor = Reflect.getOwnPropertyDescriptor(service, prop);
			if (!descriptor) return undefined;
			if ("value" in descriptor && typeof descriptor.value === "function") {
				return {
					...descriptor,
					configurable: true,
					value: descriptor.value.bind(service),
				};
			}
			return {
				...descriptor,
				configurable: true,
			};
		},
		getPrototypeOf() {
			return Reflect.getPrototypeOf(getService());
		},
	});
}
