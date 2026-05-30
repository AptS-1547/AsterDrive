import { PERSONAL_WORKSPACE } from "@/lib/workspace";
import { WorkspaceOutlet } from "./WorkspaceOutlet";

export function PersonalWorkspaceRoute() {
	return <WorkspaceOutlet workspace={PERSONAL_WORKSPACE} />;
}
