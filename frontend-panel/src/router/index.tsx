import { lazy, Suspense } from "react";
import { createBrowserRouter, Navigate, Outlet } from "react-router-dom";
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
const SearchPage = lazy(() => import("@/pages/SearchPage"));
const AdminAuditPage = lazy(() => import("@/pages/admin/AdminAuditPage"));

function Loading() {
	return (
		<div className="min-h-screen flex items-center justify-center">
			<div className="text-muted-foreground">Loading...</div>
		</div>
	);
}

function ProtectedRoute() {
	const isAuthenticated = useAuthStore((s) => s.isAuthenticated);
	const isChecking = useAuthStore((s) => s.isChecking);
	if (isChecking) return <Loading />;
	if (!isAuthenticated) return <Navigate to="/login" replace />;
	return (
		<Suspense fallback={<Loading />}>
			<Outlet />
		</Suspense>
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
		children: [{ path: "/login", element: <LoginPage /> }],
	},
	{
		element: <ProtectedRoute />,
		children: [
			{ path: "/", element: <FileBrowserPage /> },
			{ path: "/settings/webdav", element: <WebdavAccountsPage /> },
			{ path: "/trash", element: <TrashPage /> },
			{ path: "/search", element: <SearchPage /> },
		],
	},
	{
		// Public share page — no auth required
		path: "/s/:token",
		element: (
			<Suspense fallback={<Loading />}>
				<ShareViewPage />
			</Suspense>
		),
	},
	{
		element: <AdminRoute />,
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
]);
