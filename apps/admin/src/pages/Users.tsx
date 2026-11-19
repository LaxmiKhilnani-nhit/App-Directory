import { useEffect, useMemo, useState } from "react";
import { Badge, Button, EmptyState, SearchInput, Spinner } from "@app-directory/ui";
import type { AdminUser, Role } from "@app-directory/types";
import { adminApi } from "../api/admin";
import { PageHeader, Notice } from "../components/PageHeader";
import { UserEditor } from "../components/UserEditor";
import { useAsync } from "../hooks/useAsync";
import { formatRelative } from "../utils/format";

function RoleBadge({ role }: { role: Role }) {
  if (role === "super_admin") return <Badge tone="accent">Super admin</Badge>;
  return <Badge tone="neutral">User</Badge>;
}

type ActiveFilter = "all" | "active" | "inactive";

export function Users() {
  const [query, setQuery] = useState("");
  const [applied, setApplied] = useState("");
  const [role, setRole] = useState<"" | Role>("");
  const [active, setActive] = useState<ActiveFilter>("all");
  const [department, setDepartment] = useState("");
  const [editor, setEditor] = useState<{ userId: string | null } | null>(null);

  useEffect(() => {
    const handle = window.setTimeout(() => setApplied(query.trim()), 300);
    return () => window.clearTimeout(handle);
  }, [query]);

  const users = useAsync(
    () =>
      adminApi.users({
        query: applied || undefined,
        role: role || undefined,
        active: active === "all" ? undefined : active === "active",
        limit: 500,
      }),
    [applied, role, active],
  );

  const departments = useMemo(() => {
    const found = new Set<string>();
    for (const person of users.data?.users ?? []) {
      if (person.department) found.add(person.department);
    }
    return [...found].sort((a, b) => a.localeCompare(b));
  }, [users.data]);

  const rows = useMemo(() => {
    return (users.data?.users ?? []).filter((person) => {
      if (department && (person.department ?? "") !== department) return false;
      return true;
    });
  }, [users.data, department]);

  function onChanged(updated: AdminUser) {
    users.setData((previous) => {
      if (!previous) return previous;
      const exists = previous.users.some((person) => person.id === updated.id);
      return {
        ...previous,
        users: exists
          ? previous.users.map((person) => (person.id === updated.id ? updated : person))
          : [updated, ...previous.users],
      };
    });
  }

  const visibleTotal = users.data?.total ?? 0;

  return (
    <>
      <PageHeader
        title="Users & access"
        description="Accounts, roles, departments and direct app grants."
        actions={
          <Button variant="primary" onClick={() => setEditor({ userId: null })}>
            New account
          </Button>
        }
      />

      {users.error ? <Notice tone="danger">{users.error}</Notice> : null}

      <section className="admin-card">
        <div className="admin-toolbar">
          <SearchInput
            value={query}
            onValueChange={setQuery}
            placeholder="Search by name or email…"
          />

          <select
            value={role}
            onChange={(event) => setRole(event.target.value as "" | Role)}
            aria-label="Filter by role"
          >
            <option value="">All roles</option>
            <option value="user">User</option>
            <option value="super_admin">Super admin</option>
          </select>

          <select
            value={active}
            onChange={(event) => setActive(event.target.value as ActiveFilter)}
            aria-label="Filter by status"
          >
            <option value="all">All statuses</option>
            <option value="active">Active</option>
            <option value="inactive">Deactivated</option>
          </select>

          <select
            value={department}
            onChange={(event) => setDepartment(event.target.value)}
            aria-label="Filter by department"
          >
            <option value="">All departments</option>
            {departments.map((value) => (
              <option key={value} value={value}>
                {value}
              </option>
            ))}
          </select>

        </div>

        {users.loading && !users.data ? (
          <Spinner label="Loading people…" />
        ) : rows.length === 0 ? (
          <EmptyState
            title={
              users.data && users.data.users.length > 0
                ? "No people match those filters"
                : "No accounts yet"
            }
            description={
              users.data && users.data.users.length > 0
                ? "Try a broader search or clear a filter."
                : "Create an account one at a time, or bulk import a CSV."
            }
          />
        ) : (
          <div className="admin-table-wrap" style={{ marginTop: "var(--space-3)" }}>
            <table className="admin-table">
              <thead>
                <tr>
                  <th>Person</th>
                  <th>Department</th>
                  <th>Role</th>
                  <th>Status</th>
                  <th>Last sign-in</th>
                  <th aria-label="Actions" />
                </tr>
              </thead>
              <tbody>
                {rows.map((person) => (
                  <tr key={person.id}>
                    <td>
                      <div className="admin-table__name">{person.name}</div>
                      <div className="admin-table__sub admin-mono">{person.email}</div>
                    </td>
                    <td className="admin-muted-cell">{person.department ?? "—"}</td>
                    <td>
                      <RoleBadge role={person.role} />
                    </td>
                    <td>
                      {person.isActive ? (
                        <Badge tone="success">Active</Badge>
                      ) : (
                        <Badge tone="danger">Deactivated</Badge>
                      )}
                    </td>
                    <td
                      className="admin-muted-cell admin-nowrap"
                      title={person.lastLoginAt ?? undefined}
                    >
                      {person.lastLoginAt ? formatRelative(person.lastLoginAt) : "Never"}
                    </td>
                    <td>
                      <div className="admin-table__actions">
                        <Button size="sm" onClick={() => setEditor({ userId: person.id })}>
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

        <p className="admin-inline-hint" style={{ marginTop: "var(--space-3)" }}>
          Showing {rows.length} of {visibleTotal} accounts
          {visibleTotal > 500 ? " (refine the search to reach the rest)" : ""}.
        </p>
      </section>

      {editor ? (
        <UserEditor userId={editor.userId} onClose={() => setEditor(null)} onChanged={onChanged} onDeleted={users.reload} />
      ) : null}
    </>
  );
}
