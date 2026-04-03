import { beforeEach, describe, expect, it } from "vitest";
import { PERSONAL_WORKSPACE } from "@/lib/workspace";
import {
	bindWorkspaceService,
	useWorkspaceStore,
} from "@/stores/workspaceStore";

describe("bindWorkspaceService", () => {
	beforeEach(() => {
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
});
