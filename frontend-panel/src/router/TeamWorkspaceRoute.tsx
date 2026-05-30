import { Navigate, useParams } from "react-router-dom";
import { WorkspaceOutlet } from "./WorkspaceOutlet";

export function TeamWorkspaceRoute() {
	const { teamId } = useParams<{ teamId?: string }>();
	const parsedTeamId = Number(teamId);

	if (!Number.isSafeInteger(parsedTeamId) || parsedTeamId <= 0) {
		return <Navigate to="/" replace />;
	}

	return <WorkspaceOutlet workspace={{ kind: "team", teamId: parsedTeamId }} />;
}
