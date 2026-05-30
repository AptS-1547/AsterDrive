import { Suspense } from "react";
import { Navigate, Outlet } from "react-router-dom";
import { useAuthStore } from "@/stores/authStore";
import { Loading } from "./Loading";

export function LoginGuard() {
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
