import { useLayoutEffect } from "react";
import { Outlet } from "react-router-dom";
import { UploadAreaHost } from "@/components/files/UploadAreaHost";
import { type Workspace, workspaceEquals } from "@/lib/workspace";
import { useFileStore } from "@/stores/fileStore";
import { useWorkspaceStore } from "@/stores/workspaceStore";

export function WorkspaceOutlet({ workspace }: { workspace: Workspace }) {
	useLayoutEffect(() => {
		if (workspaceEquals(useWorkspaceStore.getState().workspace, workspace)) {
			return;
		}
		useWorkspaceStore.getState().setWorkspace(workspace);
		useFileStore.getState().resetWorkspaceState();
	}, [workspace]);

	return (
		<>
			<UploadAreaHost workspace={workspace} />
			<Outlet />
		</>
	);
}
