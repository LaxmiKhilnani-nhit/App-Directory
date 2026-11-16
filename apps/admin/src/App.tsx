import { Navigate, Route, Routes } from "react-router-dom";
import { AuthProvider } from "./auth";
import { Shell } from "./components/Shell";
import { PasswordGate, RequireAuth } from "./components/RequireAuth";
import { Login } from "./pages/Login";
import { ChangePassword } from "./pages/ChangePassword";
import { Dashboard } from "./pages/Dashboard";
import { Apps } from "./pages/Apps";
import { Groups } from "./pages/Groups";
import { Users } from "./pages/Users";
import { Settings } from "./pages/Settings";
import { Audit } from "./pages/Audit";

/**
 * Phase 2 shell: real sessions. Every route below `/` requires a valid bearer token;
 * `PasswordGate` sends users who still need to set a password to the change screen first.
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
          <Route path="/groups" element={<Groups />} />
          <Route path="/users" element={<Users />} />
          <Route path="/settings" element={<Settings />} />
          <Route path="/audit" element={<Audit />} />
        </Route>

        <Route path="*" element={<Navigate to="/dashboard" replace />} />
      </Routes>
    </AuthProvider>
  );
}
