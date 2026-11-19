import { useEffect, useMemo, useState } from "react";
import type { Department } from "@app-directory/types";
import { Button, SearchInput, Spinner } from "@app-directory/ui";
import { adminApi } from "../api/admin";
import { errorMessage } from "../api/errors";
import { PageHeader, Notice } from "../components/PageHeader";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { CheckboxField, TextField } from "../components/Form";
import { showToast } from "../components/Toast";
import { useAsync } from "../hooks/useAsync";
import { formatRelative } from "../utils/format";

export function Departments() {
  const departments = useAsync(() => adminApi.departments(), []);
  const apps = useAsync(() => adminApi.apps(), []);
  const [name, setName] = useState("");
  const [editing, setEditing] = useState<Department | null>(null);
  const [selectedApps, setSelectedApps] = useState<Set<string>>(new Set());
  const [appQuery, setAppQuery] = useState("");
  const [confirm, setConfirm] = useState<Department | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!editing) {
      setSelectedApps(new Set());
      return;
    }
    let active = true;
    adminApi
      .departmentApps(editing.id)
      .then((ids) => {
        if (active) setSelectedApps(new Set(ids));
      })
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, [editing]);

  const filteredApps = useMemo(() => {
    const needle = appQuery.trim().toLowerCase();
    return (apps.data ?? []).filter(
      (app) => !needle || app.name.toLowerCase().includes(needle) || (app.category ?? "").toLowerCase().includes(needle),
    );
  }, [apps.data, appQuery]);

  async function save() {
    setError(null);
    if (!name.trim()) {
      setError("A department name is required.");
      return;
    }
    setBusy(true);
    try {
      let department: Department;
      if (editing) {
        department = await adminApi.updateDepartment(editing.id, name.trim());
      } else {
        department = await adminApi.createDepartment(name.trim());
      }
      await adminApi.setDepartmentApps(department.id, [...selectedApps]);
      showToast("Saved");
      setName("");
      setEditing(null);
      setSelectedApps(new Set());
      departments.reload();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    if (!confirm) return;
    setBusy(true);
    setError(null);
    try {
      await adminApi.deleteDepartment(confirm.id);
      showToast("Department removed");
      departments.reload();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setConfirm(null);
      setBusy(false);
    }
  }

  return (
    <>
      <PageHeader
        title="Departments"
        description="Add a department and choose its apps in one place. People in the department see those apps automatically."
      />

      {error ? <Notice tone="danger">{error}</Notice> : null}
      {departments.error ? <Notice tone="danger">{departments.error}</Notice> : null}

      <section className="admin-card">
        <h2 className="admin-card__title">
          {editing ? `Edit ${editing.name}` : "New department"}
        </h2>
        <div className="admin-form-stack" style={{ marginTop: "var(--space-3)" }}>
          <TextField
            label="Name"
            value={name}
            onChange={(event) => setName(event.target.value)}
            placeholder="Finance"
            maxLength={120}
          />

          <div className="admin-stack-sm">
            <h3 className="admin-section-title">Apps for this department</h3>
            <SearchInput
              value={appQuery}
              onValueChange={setAppQuery}
              placeholder="Search apps…"
            />
            {apps.loading && !apps.data ? (
              <Spinner label="Loading apps…" />
            ) : filteredApps.length === 0 ? (
              <p className="admin-inline-hint">No apps match.</p>
            ) : (
              <div className="admin-checklist">
                {filteredApps.map((app) => (
                  <CheckboxField
                    key={app.id}
                    label={app.name}
                    hint={app.category ?? ""}
                    checked={selectedApps.has(app.id)}
                    onChange={(checked) =>
                      setSelectedApps((previous) => {
                        const next = new Set(previous);
                        if (checked) next.add(app.id);
                        else next.delete(app.id);
                        return next;
                      })
                    }
                  />
                ))}
              </div>
            )}
          </div>

          <div className="admin-actions">
            <Button variant="primary" onClick={save} disabled={busy}>
              {busy ? "Saving…" : editing ? "Save department" : "Add department with apps"}
            </Button>
            {editing ? (
              <Button
                onClick={() => {
                  setEditing(null);
                  setName("");
                  setSelectedApps(new Set());
                }}
              >
                Cancel
              </Button>
            ) : null}
          </div>
        </div>
      </section>

      <section className="admin-card">
        {departments.loading && !departments.data ? (
          <Spinner label="Loading departments…" />
        ) : !departments.data || departments.data.length === 0 ? (
          <p className="admin-inline-hint">No departments yet. Add one above.</p>
        ) : (
          <div className="admin-table-wrap">
            <table className="admin-table">
              <thead>
                <tr>
                  <th>Department</th>
                  <th>People</th>
                  <th>Apps</th>
                  <th>Updated</th>
                  <th aria-label="Actions" />
                </tr>
              </thead>
              <tbody>
                {departments.data.map((department) => (
                  <tr key={department.id}>
                    <td className="admin-table__name">{department.name}</td>
                    <td className="admin-nowrap">{department.userCount}</td>
                    <td className="admin-nowrap">{department.appCount}</td>
                    <td className="admin-muted-cell admin-nowrap">{formatRelative(department.updatedAt)}</td>
                    <td>
                      <div className="admin-table__actions">
                        <Button
                          size="sm"
                          onClick={() => {
                            setEditing(department);
                            setName(department.name);
                            setAppQuery("");
                          }}
                        >
                          Edit
                        </Button>
                        <Button variant="danger" size="sm" onClick={() => setConfirm(department)}>
                          Delete
                        </Button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>

      <ConfirmDialog
        open={Boolean(confirm)}
        title="Delete department?"
        description={`${confirm?.name ?? ""} will be removed. Its people will no longer receive apps granted to that department.`}
        confirmLabel="Delete"
        onCancel={() => setConfirm(null)}
        onConfirm={() => void remove()}
      />
    </>
  );
}
