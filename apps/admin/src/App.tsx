import type { ReactNode } from "react";
import { Navigate, Route, Routes } from "react-router-dom";
import { AuthProvider } from "./auth";
import { Shell } from "./components/Shell";
import { PasswordGate, RequireAuth, SuperAdminOnly } from "./components/RequireAuth";
import { Login } from "./pages/Login";
import { ChangePassword } from "./pages/ChangePassword";
import { Dashboard } from "./pages/Dashboard";
import { Apps } from "./pages/Apps";
import { Groups } from "./pages/Groups";
import { Users } from "./pages/Users";
import { Import } from "./pages/Import";
import { ResetRequests } from "./pages/ResetRequests";
import { Settings } from "./pages/Settings";
import { Audit } from "./pages/Audit";

function Super({ children }: { children: ReactNode }) {
  return <SuperAdminOnly>{children}</SuperAdminOnly>;
}

/**
 * Sessions are real bearer tokens; `PasswordGate` sends accounts that still need to set a
 * password to the change screen first. App admins only reach the scoped app screens, matching
 * the API's own restrictions.
 */
export function App() {
  return (
    <AuthProvider>
      <Routes>
        <Route path="/login" element={<Login />} />

        <Route
          path="/change-password"
          element={
            <RequireAuth>
              <ChangePassword />
            </RequireAuth>
          }
        />

        <Route
          element={
            <RequireAuth>
              <PasswordGate>
                <Shell />
              </PasswordGate>
            </RequireAuth>
          }
        >
          <Route index element={<Navigate to="/dashboard" replace />} />
          <Route path="/dashboard" element={<Dashboard />} />
          <Route path="/apps" element={<Apps />} />
          <Route
            path="/groups"
            element={
              <Super>
                <Groups />
              </Super>
            }
          />
          <Route
            path="/users"
            element={
              <Super>
                <Users />
              </Super>
            }
          />
          <Route
            path="/import"
            element={
              <Super>
                <Import />
              </Super>
            }
          />
          <Route
            path="/reset-requests"
            element={
              <Super>
                <ResetRequests />
              </Super>
            }
          />
          <Route
            path="/settings"
            element={
              <Super>
                <Settings />
              </Super>
            }
          />
          <Route
            path="/audit"
            element={
              <Super>
                <Audit />
              </Super>
            }
          />
        </Route>

        <Route path="*" element={<Navigate to="/dashboard" replace />} />
      </Routes>
    </AuthProvider>
  );
}
