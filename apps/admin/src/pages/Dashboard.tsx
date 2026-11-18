import { Link } from "react-router-dom";
import { Badge, EmptyState, Spinner } from "@app-directory/ui";
import { adminApi } from "../api/admin";
import { useAuth } from "../auth";
import { PageHeader, Notice } from "../components/PageHeader";
import { useAsync } from "../hooks/useAsync";
import { formatRelative } from "../utils/format";

export function Dashboard() {
  const { user } = useAuth();
  const isSuper = user?.role === "super_admin";

  const stats = useAsync(() => adminApi.stats(), []);
  const audit = useAsync(
    () => adminApi.audit(isSuper ? { limit: 8 } : { limit: 8, entityType: "app" }),
    [isSuper],
  );

  const metrics = stats.data
    ? isSuper
      ? [
          ["Users", `${stats.data.activeUsers}`, `${stats.data.users} total`],
          ["Apps", `${stats.data.enabledApps}`, `${stats.data.apps} total`],
          ["Groups", `${stats.data.groups}`, "manual + automatic"],
          ["Sign-ins today", `${stats.data.loginsToday}`, "accounts seen today"],
          ["Sync revision", `${stats.data.revision}`, "bumped on every change"],
        ]
      : [
          ["Apps in your scope", `${stats.data.enabledApps}`, `${stats.data.apps} total`],
          ["Sync revision", `${stats.data.revision}`, "bumped on every change"],
        ]
    : [];

  return (
    <>
      <PageHeader
        title={`Welcome back, ${user?.name ?? "admin"}`}
        description="What is happening in App Directory right now."
        actions={
          <Link to="/apps" className="ad-btn ad-btn--primary">
            Manage apps
          </Link>
        }
      />

      {stats.error ? <Notice tone="danger">{stats.error}</Notice> : null}

      {stats.loading && !stats.data ? (
        <div className="admin-card">
          <Spinner label="Loading dashboard…" />
        </div>
      ) : null}

      {stats.data ? (
        <div className="admin-grid">
          {metrics.map(([label, value, hint]) => (
            <div className="admin-metric" key={label}>
              <div className="admin-metric__label">{label}</div>
              <div className="admin-metric__value">{value}</div>
              <div className="admin-inline-hint">{hint}</div>
            </div>
          ))}
        </div>
      ) : null}

      <section className="admin-card">
        <div className="admin-pagehead">
          <h2 className="admin-card__title">Recent activity</h2>
          {isSuper ? (
            <Link to="/audit" className="admin-inline-hint">
              View the full audit log
            </Link>
          ) : (
            <span className="admin-inline-hint">App changes only</span>
          )}
        </div>

        {audit.error ? <Notice tone="danger">{audit.error}</Notice> : null}

        {audit.loading && !audit.data ? <Spinner label="Loading activity…" /> : null}

        {audit.data && audit.data.entries.length === 0 ? (
          <EmptyState
            title="No activity yet"
            description="Admin actions will appear here as they are recorded in the audit log."
          />
        ) : null}

        {audit.data && audit.data.entries.length > 0 ? (
          <div className="admin-table-wrap">
            <table className="admin-table">
              <thead>
                <tr>
                  <th>Action</th>
                  <th>Entity</th>
                  <th>Actor</th>
                  <th>When</th>
                </tr>
              </thead>
              <tbody>
                {audit.data.entries.map((entry) => (
                  <tr key={entry.id}>
                    <td>
                      <Badge tone="accent">{entry.action}</Badge>
                    </td>
                    <td className="admin-muted-cell">{entry.entityType}</td>
                    <td>{entry.actorEmail ?? "system"}</td>
                    <td className="admin-muted-cell admin-nowrap" title={entry.createdAt}>
                      {formatRelative(entry.createdAt)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : null}
      </section>
    </>
  );
}
