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

/// The bootstrap admin (and any admin-issued reset) must set a password before working.
export function PasswordGate({ children }: { children: ReactNode }) {
  const { user } = useAuth();
  const { pathname } = useLocation();

  if (user?.mustChangePassword && pathname !== "/change-password") {
    return <Navigate to="/change-password" replace />;
  }

  return <>{children}</>;
}
