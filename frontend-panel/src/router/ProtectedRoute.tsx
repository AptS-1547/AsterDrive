import { Suspense } from "react";
import { Navigate, Outlet } from "react-router-dom";
import { useAuthStore } from "@/stores/authStore";
import { Loading } from "./Loading";

export function ProtectedRoute() {
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
