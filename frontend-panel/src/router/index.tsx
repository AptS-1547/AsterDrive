import { lazy, Suspense } from "react";
import { createBrowserRouter, Navigate, Outlet } from "react-router-dom";
import ErrorPage from "@/pages/ErrorPage";
import { useAuthStore } from "@/stores/authStore";

const LoginPage = lazy(() => import("@/pages/LoginPage"));
const FileBrowserPage = lazy(() => import("@/pages/FileBrowserPage"));
const AdminUsersPage = lazy(() => import("@/pages/admin/AdminUsersPage"));
const AdminPoliciesPage = lazy(() => import("@/pages/admin/AdminPoliciesPage"));
const AdminSettingsPage = lazy(() => import("@/pages/admin/AdminSettingsPage"));
const AdminSharesPage = lazy(() => import("@/pages/admin/AdminSharesPage"));
const AdminLocksPage = lazy(() => import("@/pages/admin/AdminLocksPage"));
const ShareViewPage = lazy(() => import("@/pages/ShareViewPage"));
const WebdavAccountsPage = lazy(() => import("@/pages/WebdavAccountsPage"));
const TrashPage = lazy(() => import("@/pages/TrashPage"));
const SettingsPage = lazy(() => import("@/pages/SettingsPage"));
const MySharesPage = lazy(() => import("@/pages/MySharesPage"));
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
	if (isChecking) return <Loading />;
	if (!isAuthenticated) return <Navigate to="/login" replace />;
	return (
		<div className="animate-in fade-in duration-300">
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
	if (isChecking) return <Loading />;
	if (!isAuthenticated) return <Navigate to="/login" replace />;
	if (user?.role !== "admin") return <Navigate to="/" replace />;
	return (
		<Suspense fallback={<Loading />}>
			<Outlet />
		</Suspense>
	);
}

function LoginGuard() {
	const isAuthenticated = useAuthStore((s) => s.isAuthenticated);
	const isChecking = useAuthStore((s) => s.isChecking);
	if (isChecking) return <Loading />;
	if (isAuthenticated) return <Navigate to="/" replace />;
	return (
		<Suspense fallback={<Loading />}>
			<Outlet />
		</Suspense>
	);
}

export const router = createBrowserRouter([
	{
		element: <LoginGuard />,
		errorElement: <ErrorPage />,
		children: [{ path: "/login", element: <LoginPage /> }],
	},
	{
		element: <ProtectedRoute />,
		errorElement: <ErrorPage />,
		children: [
			{ path: "/", element: <FileBrowserPage /> },
			{ path: "/folder/:folderId", element: <FileBrowserPage /> },
			{ path: "/shares", element: <MySharesPage /> },
			{ path: "/settings/webdav", element: <WebdavAccountsPage /> },
			{ path: "/trash", element: <TrashPage /> },
			{ path: "/settings", element: <SettingsPage /> },
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
			{ path: "/admin", element: <Navigate to="/admin/users" replace /> },
			{ path: "/admin/users", element: <AdminUsersPage /> },
			{ path: "/admin/policies", element: <AdminPoliciesPage /> },
			{ path: "/admin/shares", element: <AdminSharesPage /> },
			{ path: "/admin/locks", element: <AdminLocksPage /> },
			{ path: "/admin/settings", element: <AdminSettingsPage /> },
			{ path: "/admin/audit", element: <AdminAuditPage /> },
		],
	},
	{ path: "*", element: <ErrorPage /> },
]);
