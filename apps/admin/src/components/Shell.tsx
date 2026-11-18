import { NavLink, Outlet, useLocation, useNavigate } from "react-router-dom";
import { Button } from "@app-directory/ui";
import { useAuth } from "../auth";

interface NavItem {
  to: string;
  label: string;
  superOnly?: boolean;
}

const NAV: NavItem[] = [
  { to: "/dashboard", label: "Dashboard" },
  { to: "/apps", label: "Apps" },
  { to: "/groups", label: "Groups", superOnly: true },
  { to: "/users", label: "Users", superOnly: true },
  { to: "/import", label: "CSV import", superOnly: true },
  { to: "/settings", label: "Settings", superOnly: true },
  { to: "/audit", label: "Audit log", superOnly: true },
];

const TITLES: Record<string, string> = {
  "/dashboard": "Dashboard",
  "/apps": "Applications",
  "/groups": "Groups",
  "/users": "Users & access",
  "/import": "CSV import",
  "/reset-requests": "Password reset requests",
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
  const isSuper = user?.role === "super_admin";

  const items = NAV.filter((item) => !item.superOnly || isSuper);

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
          {items.map((item) => (
            <NavLink key={item.to} to={item.to}>
              {item.label}
            </NavLink>
          ))}
          <a href="/download/app-directory.exe" className="admin-nav__download">
            Download client
          </a>
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
