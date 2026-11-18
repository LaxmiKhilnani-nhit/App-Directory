import type { ReactNode } from "react";
import { Navigate, Route, Routes } from "react-router-dom";
import { AuthProvider } from "./auth";
import { Shell } from "./components/Shell";
import { RequireAuth, SuperAdminOnly } from "./components/RequireAuth";
import { Login } from "./pages/Login";
import { Dashboard } from "./pages/Dashboard";
import { Apps } from "./pages/Apps";
import { Groups } from "./pages/Groups";
import { Users } from "./pages/Users";
import { Import } from "./pages/Import";
import { Settings } from "./pages/Settings";
import { Audit } from "./pages/Audit";

function Super({ children }: { children: ReactNode }) {
  return <SuperAdminOnly>{children}</SuperAdminOnly>;
}

/**
 * The admin portal keeps a sign-in screen for administrators. The desktop client does not:
 * it is identified by the current Windows user (see README.md).
 */
export function App() {
  return (
    <AuthProvider>
      <Routes>
        <Route path="/login" element={<Login />} />

        <Route
          element={
            <RequireAuth>
              <Shell />
            </RequireAuth>
          }
        >
          <Route index element={<Navigate to="/dashboard" replace />} />
          <Route path="/dashboard" element={<Dashboard />} />
          <Route path="/apps" element={<Apps />} />
          <Route path="/groups" element={<Super><Groups /></Super>} />
          <Route path="/users" element={<Super><Users /></Super>} />
          <Route path="/import" element={<Super><Import /></Super>} />
          <Route path="/settings" element={<Super><Settings /></Super>} />
          <Route path="/audit" element={<Super><Audit /></Super>} />
        </Route>

        <Route path="*" element={<Navigate to="/dashboard" replace />} />
      </Routes>
    </AuthProvider>
  );
}
