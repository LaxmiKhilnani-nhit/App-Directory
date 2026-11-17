import { useMemo, useState } from "react";
import { Badge, Button, EmptyState, SearchInput, Spinner } from "@app-directory/ui";
import type { AdminApp } from "@app-directory/types";
import { adminApi } from "../api/admin";
import { AppEditor } from "../components/AppEditor";
import { PageHeader, Notice } from "../components/PageHeader";
import { useAsync } from "../hooks/useAsync";
import { useIconUrl } from "../hooks/useIconUrl";
import { fallbackColor, initials } from "../utils/format";

function AppThumb({ app }: { app: AdminApp }) {
  const url = useIconUrl(app.id, app.iconEtag);

  if (url) {
    return <img className="admin-thumb" src={url} alt="" />;
  }

  return (
    <span
      className="admin-thumb admin-thumb--letter"
      style={{ background: app.color ?? fallbackColor(app.name) }}
      aria-hidden="true"
    >
      {initials(app.name)}
    </span>
  );
}

type StatusFilter = "all" | "enabled" | "disabled";

export function Apps() {
  const apps = useAsync(() => adminApi.apps(), []);
  const groups = useAsync(() => adminApi.groups(), []);

  const [query, setQuery] = useState("");
  const [status, setStatus] = useState<StatusFilter>("all");
  const [category, setCategory] = useState("");
  const [editor, setEditor] = useState<{ app: AdminApp | null } | null>(null);

  const rows = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return (apps.data ?? []).filter((app) => {
      if (status === "enabled" && !app.isEnabled) return false;
      if (status === "disabled" && app.isEnabled) return false;
      if (category && (app.category ?? "") !== category) return false;
      if (!needle) return true;
      return [app.name, app.url, app.category ?? "", app.description ?? ""].some((value) =>
        value.toLowerCase().includes(needle),
      );
    });
  }, [apps.data, query, status, category]);

  const categories = useMemo(() => {
    const found = new Set<string>();
    for (const app of apps.data ?? []) if (app.category) found.add(app.category);
    return [...found].sort((a, b) => a.localeCompare(b));
  }, [apps.data]);

  function onChanged(updated: AdminApp | null) {
    if (!updated) {
      const deletedId = editor?.app?.id;
      if (deletedId) apps.setData((apps.data ?? []).filter((app) => app.id !== deletedId));
      return;
    }

    apps.setData(
      (apps.data ?? []).some((app) => app.id === updated.id)
        ? (apps.data ?? []).map((app) => (app.id === updated.id ? updated : app))
        : [...(apps.data ?? []), updated],
    );
  }

  return (
    <>
      <PageHeader
        title="Applications"
        description="Tiles that appear in the desktop client. The server decides who can see each one."
        actions={
          <Button variant="primary" onClick={() => setEditor({ app: null })}>
            New app
          </Button>
        }
      />

      {apps.error ? <Notice tone="danger">{apps.error}</Notice> : null}
      {groups.error ? <Notice tone="danger">{groups.error}</Notice> : null}

      <section className="admin-card">
        <div className="admin-toolbar">
          <SearchInput
            value={query}
            onValueChange={setQuery}
            placeholder="Search by name, URL or category…"
          />

          <select
            value={status}
            onChange={(event) => setStatus(event.target.value as StatusFilter)}
            aria-label="Filter by status"
          >
            <option value="all">All statuses</option>
            <option value="enabled">Enabled</option>
            <option value="disabled">Disabled</option>
          </select>

          <select
            value={category}
            onChange={(event) => setCategory(event.target.value)}
            aria-label="Filter by category"
          >
            <option value="">All categories</option>
            {categories.map((value) => (
              <option key={value} value={value}>
                {value}
              </option>
            ))}
          </select>
        </div>

        {apps.loading && !apps.data ? (
          <Spinner label="Loading apps…" />
        ) : rows.length === 0 ? (
          <EmptyState
            title={
              apps.data && apps.data.length > 0 ? "No apps match those filters" : "No apps yet"
            }
            description={
              apps.data && apps.data.length > 0
                ? "Clear the search or pick a different category."
                : "Add the first tile, then grant it to groups or individual people."
            }
          />
        ) : (
          <div className="admin-table-wrap" style={{ marginTop: "var(--space-3)" }}>
            <table className="admin-table">
              <thead>
                <tr>
                  <th>Application</th>
                  <th>Category</th>
                  <th>Status</th>
                  <th>Groups</th>
                  <th>People</th>
                  <th>Sort</th>
                  <th aria-label="Actions" />
                </tr>
              </thead>
              <tbody>
                {rows.map((app) => (
                  <tr key={app.id}>
                    <td>
                      <div className="admin-table__primary">
                        <AppThumb app={app} />
                        <div style={{ minWidth: 0 }}>
                          <div className="admin-table__name">{app.name}</div>
                          <div className="admin-table__sub" title={app.url}>
                            {app.url}
                          </div>
                        </div>
                      </div>
                    </td>
                    <td className="admin-muted-cell">{app.category ?? "—"}</td>
                    <td>
                      {app.isEnabled ? (
                        <Badge tone="success">Enabled</Badge>
                      ) : (
                        <Badge tone="neutral">Disabled</Badge>
                      )}
                    </td>
                    <td className="admin-nowrap">{app.groupIds.length}</td>
                    <td className="admin-nowrap">{app.userIds.length}</td>
                    <td className="admin-mono admin-muted-cell">{app.sortOrder}</td>
                    <td>
                      <div className="admin-table__actions">
                        <Button size="sm" onClick={() => setEditor({ app })}>
                          Edit
                        </Button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        {apps.data ? (
          <p className="admin-inline-hint" style={{ marginTop: "var(--space-3)" }}>
            Showing {rows.length} of {apps.data.length} apps.
          </p>
        ) : null}
      </section>

      {editor ? (
        <AppEditor
          app={editor.app}
          groups={groups.data ?? []}
          onClose={() => setEditor(null)}
          onChanged={onChanged}
        />
      ) : null}
    </>
  );
}
