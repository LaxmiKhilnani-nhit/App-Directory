import { NavLink, Outlet, useLocation, useNavigate } from "react-router-dom";
import { Button } from "@app-directory/ui";
import { useAuth } from "../auth";

const NAV = [
  { to: "/dashboard", label: "Dashboard" },
  { to: "/apps", label: "Apps" },
  { to: "/groups", label: "Groups" },
  { to: "/users", label: "Users" },
  { to: "/settings", label: "Settings" },
  { to: "/audit", label: "Audit log" },
];

const TITLES: Record<string, string> = {
  "/dashboard": "Dashboard",
  "/apps": "Applications",
  "/groups": "Groups",
  "/users": "Users & access",
  "/settings": "Settings",
  "/audit": "Audit log",
};

const ROLE_LABEL: Record<string, string> = {
  super_admin: "Super admin",
  app_admin: "App admin",
  user: "User",
};

export function Shell() {
  const { pathname } = useLocation();
  const navigate = useNavigate();
  const { user, signOut } = useAuth();

  async function onSignOut() {
    await signOut();
    navigate("/login", { replace: true });
  }

  return (
    <div className="admin-shell">
      <aside className="admin-sidebar">
        <div className="admin-brand">
          <div>
            <div className="admin-brand__name">App Directory</div>
            <div className="admin-brand__sub">Admin</div>
          </div>
        </div>
        <nav className="admin-nav">
          {NAV.map((item) => (
            <NavLink key={item.to} to={item.to}>
              {item.label}
            </NavLink>
          ))}
        </nav>
      </aside>
      <div className="admin-main">
        <header className="admin-topbar">
          <h1 className="admin-topbar__title">{TITLES[pathname] ?? "App Directory"}</h1>
          <div className="admin-user">
            <span className="admin-user__name" title={user?.email}>
              {user?.name}
            </span>
            <span className="ad-badge ad-badge--accent">
              {ROLE_LABEL[user?.role ?? "user"] ?? user?.role}
            </span>
            <Button variant="ghost" size="sm" onClick={onSignOut}>
              Sign out
            </Button>
          </div>
        </header>
        <main className="admin-content">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
