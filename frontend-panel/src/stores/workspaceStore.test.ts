import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PERSONAL_WORKSPACE } from "@/lib/workspace";
import {
	bindWorkspaceService,
	useWorkspaceStore,
} from "@/stores/workspaceStore";

describe("bindWorkspaceService", () => {
	beforeEach(() => {
		useWorkspaceStore.getState().setWorkspace(PERSONAL_WORKSPACE);
	});

	afterEach(() => {
		useWorkspaceStore.getState().setWorkspace(PERSONAL_WORKSPACE);
	});

	it("mirrors the current service shape for introspection", () => {
		const service = bindWorkspaceService((workspace) => ({
			workspaceKey:
				workspace.kind === "team" ? `team:${workspace.teamId}` : "personal",
			describe() {
				return this.workspaceKey;
			},
		}));

		expect("workspaceKey" in service).toBe(true);
		expect("describe" in service).toBe(true);
		expect(Object.keys(service)).toEqual(["workspaceKey", "describe"]);

		const enumeratedKeys: string[] = [];
		for (const key in service) {
			enumeratedKeys.push(key);
		}
		expect(enumeratedKeys).toEqual(["workspaceKey", "describe"]);

		const descriptor = Object.getOwnPropertyDescriptor(service, "describe");
		expect(descriptor?.enumerable).toBe(true);
		expect(descriptor?.configurable).toBe(true);
		expect(service.describe()).toBe("personal");

		useWorkspaceStore.getState().setWorkspace({ kind: "team", teamId: 7 });

		expect(service.workspaceKey).toBe("team:7");
		expect(service.describe()).toBe("team:7");
	});

	it("uses the current workspace for cached method references", () => {
		const service = bindWorkspaceService((workspace) => ({
			deleteFile(id: number) {
				const prefix =
					workspace.kind === "team" ? `/teams/${workspace.teamId}` : "";
				return `${prefix}/files/${id}`;
			},
		}));

		const remove = service.deleteFile;
		const { deleteFile } = service;

		expect(remove(8)).toBe("/files/8");
		expect(deleteFile(9)).toBe("/files/9");

		useWorkspaceStore.getState().setWorkspace({ kind: "team", teamId: 7 });

		expect(remove(8)).toBe("/teams/7/files/8");
		expect(deleteFile(9)).toBe("/teams/7/files/9");
	});

	it("does not notify subscribers when setting an equal workspace", () => {
		const subscriber = vi.fn();
		const unsubscribe = useWorkspaceStore.subscribe(subscriber);

		useWorkspaceStore.getState().setWorkspace(PERSONAL_WORKSPACE);

		expect(subscriber).not.toHaveBeenCalled();
		unsubscribe();
	});

	it("rejects direct property assignment on the proxy", () => {
		const service = bindWorkspaceService(() => ({
			workspaceKey: "personal",
		}));

		const mutateWorkspaceKey = () => {
			(
				service as unknown as {
					workspaceKey: string;
				}
			).workspaceKey = "mutated";
		};

		expect(mutateWorkspaceKey).toThrowError(TypeError);
		expect(mutateWorkspaceKey).toThrow(/Cannot set property "workspaceKey"/);
		expect(service.workspaceKey).toBe("personal");
	});
});
