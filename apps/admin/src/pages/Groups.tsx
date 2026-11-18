import { useMemo, useState } from "react";
import { Badge, Button, EmptyState, SearchInput, Spinner } from "@app-directory/ui";
import type { AdminGroup } from "@app-directory/types";
import { adminApi } from "../api/admin";
import { GroupEditor } from "../components/GroupEditor";
import { PageHeader, Notice } from "../components/PageHeader";
import { useAsync } from "../hooks/useAsync";
import { formatRelative } from "../utils/format";

type TypeFilter = "all" | "manual" | "auto";

function groupKind(group: AdminGroup): Exclude<TypeFilter, "all"> {
  if (group.autoDepartment) return "auto";
  return "manual";
}

function KindBadge({ group }: { group: AdminGroup }) {
  if (group.autoDepartment) return <Badge tone="warning">Auto · department</Badge>;
  return <Badge tone="neutral">Manual</Badge>;
}

export function Groups() {
  const groups = useAsync(() => adminApi.groups(), []);

  const [query, setQuery] = useState("");
  const [type, setType] = useState<TypeFilter>("all");
  const [editor, setEditor] = useState<{ group: AdminGroup | null } | null>(null);

  const rows = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return (groups.data ?? []).filter((group) => {
      if (type !== "all" && groupKind(group) !== type) return false;
      if (!needle) return true;
      return [group.name, group.description ?? "", group.autoDepartment ?? ""].some((value) =>
        value.toLowerCase().includes(needle),
      );
    });
  }, [groups.data, query, type]);

  function onChanged(updated: AdminGroup | null) {
    if (!updated) {
      const deletedId = editor?.group?.id;
      if (deletedId) groups.setData((groups.data ?? []).filter((group) => group.id !== deletedId));
      return;
    }

    groups.setData(
      (groups.data ?? []).some((group) => group.id === updated.id)
        ? (groups.data ?? []).map((group) => (group.id === updated.id ? updated : group))
        : [...(groups.data ?? []), updated],
    );
  }

  return (
    <>
      <PageHeader
        title="Groups"
        description="Manual lists of people. Grant apps to a group to give every member access."
        actions={
          <Button variant="primary" onClick={() => setEditor({ group: null })}>
            New group
          </Button>
        }
      />

      {groups.error ? <Notice tone="danger">{groups.error}</Notice> : null}

      <section className="admin-card">
        <div className="admin-toolbar">
          <SearchInput
            value={query}
            onValueChange={setQuery}
            placeholder="Search groups by name or department…"
          />

          <select
            value={type}
            onChange={(event) => setType(event.target.value as TypeFilter)}
            aria-label="Filter by group type"
          >
            <option value="all">All types</option>
            <option value="manual">Manual</option>
            <option value="auto">Automatic by department</option>
            <option value="all_users">All Users</option>
          </select>
        </div>

        {groups.loading && !groups.data ? (
          <Spinner label="Loading groups…" />
        ) : rows.length === 0 ? (
          <EmptyState
            title={
              groups.data && groups.data.length > 0
                ? "No groups match those filters"
                : "No groups yet"
            }
            description={
              groups.data && groups.data.length > 0
                ? "Try a different search or group type."
                : "Create a manual group, or one that follows a department automatically."
            }
          />
        ) : (
          <div className="admin-table-wrap" style={{ marginTop: "var(--space-3)" }}>
            <table className="admin-table">
              <thead>
                <tr>
                  <th>Group</th>
                  <th>Type</th>
                  <th>Members</th>
                  <th>Apps</th>
                  <th>Updated</th>
                  <th aria-label="Actions" />
                </tr>
              </thead>
              <tbody>
                {rows.map((group) => (
                  <tr key={group.id}>
                    <td>
                      <div className="admin-table__name">{group.name}</div>
                      <div className="admin-table__sub">
                        {group.description ??
                          (group.autoDepartment
                            ? `Department: ${group.autoDepartment}`
                            : "No description")}
                      </div>
                    </td>
                    <td>
                      <KindBadge group={group} />
                    </td>
                    <td className="admin-nowrap">{group.memberCount}</td>
                    <td className="admin-nowrap">{group.appCount}</td>
                    <td className="admin-muted-cell admin-nowrap" title={group.updatedAt}>
                      {formatRelative(group.updatedAt)}
                    </td>
                    <td>
                      <div className="admin-table__actions">
                        <Button size="sm" onClick={() => setEditor({ group })}>
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

        {groups.data ? (
          <p className="admin-inline-hint" style={{ marginTop: "var(--space-3)" }}>
            Showing {rows.length} of {groups.data.length} groups.
          </p>
        ) : null}
      </section>

      {editor ? (
        <GroupEditor group={editor.group} onClose={() => setEditor(null)} onChanged={onChanged} />
      ) : null}
    </>
  );
}
