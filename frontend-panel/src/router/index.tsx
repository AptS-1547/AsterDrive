import { lazy, Suspense, useLayoutEffect } from "react";
import {
	createBrowserRouter,
	Navigate,
	Outlet,
	useParams,
} from "react-router-dom";
import { AdminSiteUrlMismatchPrompt } from "@/components/layout/AdminSiteUrlMismatchPrompt";
import {
	PERSONAL_WORKSPACE,
	type Workspace,
	workspaceEquals,
} from "@/lib/workspace";
import ErrorPage from "@/pages/ErrorPage";
import { useAuthStore } from "@/stores/authStore";
import { useFileStore } from "@/stores/fileStore";
import { useWorkspaceStore } from "@/stores/workspaceStore";

const LoginPage = lazy(() => import("@/pages/LoginPage"));
const ResetPasswordPage = lazy(() => import("@/pages/ResetPasswordPage"));
const FileBrowserPage = lazy(() => import("@/pages/FileBrowserPage"));
const AdminOverviewPage = lazy(() => import("@/pages/admin/AdminOverviewPage"));
const AdminUsersPage = lazy(() => import("@/pages/admin/AdminUsersPage"));
const AdminTeamsPage = lazy(() => import("@/pages/admin/AdminTeamsPage"));
const AdminTeamDetailPage = lazy(
	() => import("@/pages/admin/AdminTeamDetailPage"),
);
const AdminPoliciesPage = lazy(() => import("@/pages/admin/AdminPoliciesPage"));
const AdminPolicyGroupsPage = lazy(
	() => import("@/pages/admin/AdminPolicyGroupsPage"),
);
const AdminTasksPage = lazy(() => import("@/pages/admin/AdminTasksPage"));
const AdminSettingsPage = lazy(() => import("@/pages/admin/AdminSettingsPage"));
const AdminSharesPage = lazy(() => import("@/pages/admin/AdminSharesPage"));
const AdminLocksPage = lazy(() => import("@/pages/admin/AdminLocksPage"));
const AdminAboutPage = lazy(() => import("@/pages/admin/AdminAboutPage"));
const ShareViewPage = lazy(() => import("@/pages/ShareViewPage"));
const WebdavAccountsPage = lazy(() => import("@/pages/WebdavAccountsPage"));
const TrashPage = lazy(() => import("@/pages/TrashPage"));
const SettingsPage = lazy(() => import("@/pages/SettingsPage"));
const TeamManagePage = lazy(() => import("@/pages/TeamManagePage"));
const MySharesPage = lazy(() => import("@/pages/MySharesPage"));
const TasksPage = lazy(() => import("@/pages/TasksPage"));
const AdminAuditPage = lazy(() => import("@/pages/admin/AdminAuditPage"));

function Loading() {
	return (
		<div className="min-h-screen flex items-center justify-center animate-in fade-in duration-500">
			<div className="h-5 w-5 border-2 border-muted-foreground/30 border-t-muted-foreground rounded-full animate-spin" />
		</div>
	);
}

function ProtectedRoute() {
	const isAuthenticated = useAuthStore((s) => s.isAuthenticated);
	const isChecking = useAuthStore((s) => s.isChecking);
	if (!isAuthenticated && isChecking) return <Loading />;
	if (!isAuthenticated) return <Navigate to="/login" replace />;
	return (
		<div
			className="animate-in fade-in duration-300"
			aria-busy={isChecking || undefined}
		>
			<Suspense fallback={<Loading />}>
				<Outlet />
			</Suspense>
		</div>
	);
}

function AdminRoute() {
	const user = useAuthStore((s) => s.user);
	const isChecking = useAuthStore((s) => s.isChecking);
	const isAuthenticated = useAuthStore((s) => s.isAuthenticated);
	if (!isAuthenticated && isChecking) return <Loading />;
	if (!isAuthenticated) return <Navigate to="/login" replace />;
	if (!user && isChecking) return <Loading />;
	if (user?.role !== "admin") return <Navigate to="/" replace />;
	return (
		<div aria-busy={isChecking || undefined}>
			<Suspense fallback={<Loading />}>
				<AdminSiteUrlMismatchPrompt />
				<Outlet />
			</Suspense>
		</div>
	);
}

function LoginGuard() {
	const isAuthenticated = useAuthStore((s) => s.isAuthenticated);
	const isChecking = useAuthStore((s) => s.isChecking);
	if (isAuthenticated) return <Navigate to="/" replace />;
	if (isChecking) return <Loading />;
	return (
		<Suspense fallback={<Loading />}>
			<Outlet />
		</Suspense>
	);
}

function WorkspaceOutlet({ workspace }: { workspace: Workspace }) {
	useLayoutEffect(() => {
		if (workspaceEquals(useWorkspaceStore.getState().workspace, workspace)) {
			return;
		}
		useWorkspaceStore.getState().setWorkspace(workspace);
		useFileStore.getState().resetWorkspaceState();
	}, [workspace]);

	return <Outlet />;
}

function PersonalWorkspaceRoute() {
	return <WorkspaceOutlet workspace={PERSONAL_WORKSPACE} />;
}

function TeamWorkspaceRoute() {
	const { teamId } = useParams<{ teamId?: string }>();
	const parsedTeamId = Number(teamId);

	if (!Number.isSafeInteger(parsedTeamId) || parsedTeamId <= 0) {
		return <Navigate to="/" replace />;
	}

	return <WorkspaceOutlet workspace={{ kind: "team", teamId: parsedTeamId }} />;
}

export const router = createBrowserRouter([
	{
		element: <LoginGuard />,
		errorElement: <ErrorPage />,
		children: [{ path: "/login", element: <LoginPage /> }],
	},
	{
		path: "/reset-password",
		errorElement: <ErrorPage />,
		element: (
			<Suspense fallback={<Loading />}>
				<ResetPasswordPage />
			</Suspense>
		),
	},
	{
		element: <ProtectedRoute />,
		errorElement: <ErrorPage />,
		children: [
			{
				element: <PersonalWorkspaceRoute />,
				children: [
					{ path: "/", element: <FileBrowserPage /> },
					{ path: "/folder/:folderId", element: <FileBrowserPage /> },
					{ path: "/shares", element: <MySharesPage /> },
					{ path: "/tasks", element: <TasksPage /> },
					{ path: "/settings/webdav", element: <WebdavAccountsPage /> },
					{ path: "/trash", element: <TrashPage /> },
					{
						path: "/settings",
						element: <Navigate to="/settings/profile" replace />,
					},
					{
						path: "/settings/profile",
						element: <SettingsPage section="profile" />,
					},
					{
						path: "/settings/interface",
						element: <SettingsPage section="interface" />,
					},
					{
						path: "/settings/security",
						element: <SettingsPage section="security" />,
					},
					{
						path: "/settings/teams",
						element: <SettingsPage section="teams" />,
					},
					{
						path: "/settings/teams/:teamId",
						element: <TeamManagePage />,
					},
					{
						path: "/settings/teams/:teamId/:section",
						element: <TeamManagePage />,
					},
				],
			},
			{
				path: "/teams/:teamId",
				element: <TeamWorkspaceRoute />,
				children: [
					{ index: true, element: <FileBrowserPage /> },
					{ path: "folder/:folderId", element: <FileBrowserPage /> },
					{ path: "shares", element: <MySharesPage /> },
					{ path: "tasks", element: <TasksPage /> },
					{ path: "trash", element: <TrashPage /> },
				],
			},
		],
	},
	{
		// Public share page — no auth required
		path: "/s/:token",
		errorElement: <ErrorPage />,
		element: (
			<Suspense fallback={<Loading />}>
				<ShareViewPage />
			</Suspense>
		),
	},
	{
		element: <AdminRoute />,
		errorElement: <ErrorPage />,
		children: [
			{ path: "/admin", element: <Navigate to="/admin/overview" replace /> },
			{ path: "/admin/overview", element: <AdminOverviewPage /> },
			{ path: "/admin/users", element: <AdminUsersPage /> },
			{ path: "/admin/teams", element: <AdminTeamsPage /> },
			{ path: "/admin/teams/:teamId", element: <AdminTeamDetailPage /> },
			{
				path: "/admin/teams/:teamId/:section",
				element: <AdminTeamDetailPage />,
			},
			{ path: "/admin/policies", element: <AdminPoliciesPage /> },
			{ path: "/admin/policy-groups", element: <AdminPolicyGroupsPage /> },
			{ path: "/admin/shares", element: <AdminSharesPage /> },
			{ path: "/admin/tasks", element: <AdminTasksPage /> },
			{ path: "/admin/locks", element: <AdminLocksPage /> },
			{
				path: "/admin/settings",
				element: <Navigate to="/admin/settings/general" replace />,
			},
			{
				path: "/admin/settings/auth",
				element: <AdminSettingsPage section="auth" />,
			},
			{
				path: "/admin/settings/mail",
				element: <AdminSettingsPage section="mail" />,
			},
			{
				path: "/admin/settings/user",
				element: <AdminSettingsPage section="user" />,
			},
			{
				path: "/admin/settings/network",
				element: <AdminSettingsPage section="network" />,
			},
			{
				path: "/admin/settings/storage",
				element: <AdminSettingsPage section="storage" />,
			},
			{
				path: "/admin/settings/webdav",
				element: <AdminSettingsPage section="webdav" />,
			},
			{
				path: "/admin/settings/audit",
				element: <AdminSettingsPage section="audit" />,
			},
			{
				path: "/admin/settings/general",
				element: <AdminSettingsPage section="general" />,
			},
			{
				path: "/admin/settings/operations",
				element: <AdminSettingsPage section="operations" />,
			},
			{
				path: "/admin/settings/custom",
				element: <AdminSettingsPage section="custom" />,
			},
			{
				path: "/admin/settings/other",
				element: <AdminSettingsPage section="other" />,
			},
			{
				path: "/admin/settings/:section",
				element: <Navigate to="/admin/settings/general" replace />,
			},
			{ path: "/admin/audit", element: <AdminAuditPage /> },
			{ path: "/admin/about", element: <AdminAboutPage /> },
		],
	},
	{ path: "*", element: <Navigate to="/" replace /> },
]);
