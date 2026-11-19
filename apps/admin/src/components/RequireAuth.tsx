import type { ReactNode } from "react";
import { Navigate, useLocation } from "react-router-dom";
import { Spinner } from "@app-directory/ui";
import { useAuth } from "../auth";

export function RequireAuth({ children }: { children: ReactNode }) {
  const { user, loading } = useAuth();
  const location = useLocation();

  if (loading) {
    return (
      <div className="admin-loading">
        <Spinner label="Checking your session…" />
      </div>
    );
  }

  if (!user) {
    return <Navigate to="/login" replace state={{ from: location.pathname }} />;
  }

  return <>{children}</>;
}

/// Server screens for departments, users, imports, settings and audit are super-admin only.
/// App admins are redirected to the app list their scope can actually reach.
export function SuperAdminOnly({ children }: { children: ReactNode }) {
  const { user } = useAuth();

  if (user?.role !== "super_admin") {
    return <Navigate to="/apps" replace />;
  }

  return <>{children}</>;
}
