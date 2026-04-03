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
			workspaceEquals(state.workspace, workspace) ? state : { workspace },
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
	const wrappedMethods = new Map<PropertyKey, unknown>();

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

	function getWrappedMethod(prop: PropertyKey) {
		if (wrappedMethods.has(prop)) {
			return wrappedMethods.get(prop);
		}

		const wrapped = (...args: unknown[]) => {
			const service = getService();
			const value = service[prop as keyof T];
			if (typeof value !== "function") {
				throw new TypeError(`Property ${String(prop)} is not callable`);
			}
			return Reflect.apply(value, service, args);
		};
		wrappedMethods.set(prop, wrapped);
		return wrapped;
	}

	return new Proxy({} as T, {
		get(_target, prop) {
			const service = getService();
			const value = service[prop as keyof T];
			return typeof value === "function" ? getWrappedMethod(prop) : value;
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
					value: getWrappedMethod(prop),
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
