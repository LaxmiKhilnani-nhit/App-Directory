import { Navigate, Route, Routes } from "react-router-dom";
import { Shell } from "./components/Shell";
import { Login } from "./pages/Login";
import { Dashboard } from "./pages/Dashboard";
import { Apps } from "./pages/Apps";
import { Groups } from "./pages/Groups";
import { Users } from "./pages/Users";
import { Settings } from "./pages/Settings";
import { Audit } from "./pages/Audit";

/**
 * Phase 0 shell: routes and layout only. Authentication guarding arrives with the
 * auth API in Phase 2 (`/admin/me`), at which point <RequireAuth> wraps the shell.
 */
export function App() {
  return (
    <Routes>
      <Route path="/login" element={<Login />} />
      <Route element={<Shell />}>
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
  );
}
