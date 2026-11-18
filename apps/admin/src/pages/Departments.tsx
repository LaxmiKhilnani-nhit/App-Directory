import { useState } from "react";
import type { Department } from "@app-directory/types";
import { Button, Spinner } from "@app-directory/ui";
import { adminApi } from "../api/admin";
import { errorMessage } from "../api/errors";
import { PageHeader, Notice } from "../components/PageHeader";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { TextField } from "../components/Form";
import { useAsync } from "../hooks/useAsync";
import { formatRelative } from "../utils/format";

export function Departments() {
  const departments = useAsync(() => adminApi.departments(), []);
  const [name, setName] = useState("");
  const [editing, setEditing] = useState<Department | null>(null);
  const [confirm, setConfirm] = useState<Department | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  async function save() {
    setError(null);
    setNotice(null);
    if (!name.trim()) {
      setError("A department name is required.");
      return;
    }
    setBusy(true);
    try {
      if (editing) {
        await adminApi.updateDepartment(editing.id, name.trim());
        setNotice("Department renamed. Its people and app grants were kept.");
      } else {
        await adminApi.createDepartment(name.trim());
        setNotice("Department created.");
      }
      setName("");
      setEditing(null);
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
      setNotice("Department removed. Its people lost the department's app grants.");
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
        description="Departments group people for app access. Grant an app to a department to give it to everyone in it."
      />

      {error ? <Notice tone="danger">{error}</Notice> : null}
      {notice ? <Notice tone="success">{notice}</Notice> : null}
      {departments.error ? <Notice tone="danger">{departments.error}</Notice> : null}

      <section className="admin-card">
        <h2 className="admin-card__title">{editing ? "Rename department" : "New department"}</h2>
        <div className="admin-form-stack" style={{ marginTop: "var(--space-3)" }}>
          <TextField
            label="Name"
            value={name}
            onChange={(event) => setName(event.target.value)}
            placeholder="Finance"
            maxLength={120}
          />
          <div className="admin-actions">
            <Button variant="primary" onClick={save} disabled={busy}>{editing ? "Save" : "Add department"}</Button>
            {editing ? (
              <Button onClick={() => { setEditing(null); setName(""); }}>Cancel</Button>
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
                          }}
                        >
                          Rename
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
