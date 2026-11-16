import { NavLink, Outlet, useLocation } from "react-router-dom";

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

export function Shell() {
  const { pathname } = useLocation();

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
          <span className="ad-badge">Phase 0</span>
        </header>
        <main className="admin-content">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
