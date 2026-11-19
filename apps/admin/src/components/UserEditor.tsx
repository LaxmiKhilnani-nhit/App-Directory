import { useEffect, useState } from "react";
import { Badge, Button, Spinner } from "@app-directory/ui";
import type { AdminUser, Role, UserDetail } from "@app-directory/types";
import { adminApi, type UserInput, type UserPatch } from "../api/admin";
import { errorMessage } from "../api/errors";
import { useAuth } from "../auth";
import { ConfirmDialog } from "./ConfirmDialog";
import { Drawer } from "./Drawer";
import { CheckboxField, SelectField, TextField } from "./Form";
import { Notice } from "./PageHeader";
import { showToast } from "./Toast";
import { useAsync } from "../hooks/useAsync";

interface UserEditorProps {
  userId: string | null;
  onClose: () => void;
  onChanged: (user: AdminUser) => void;
  onDeleted?: () => void;
}

const ROLE_LABEL: Record<Role, string> = {
  user: "User",
  super_admin: "Super admin",
};

export function UserEditor({ userId, onClose, onChanged, onDeleted }: UserEditorProps) {
  const { user: me } = useAuth();

  const effectiveId = userId;

  const detail = useAsync<UserDetail | null>(
    () => (effectiveId ? adminApi.user(effectiveId) : Promise.resolve(null)),
    [effectiveId],
  );

  const apps = useAsync(() => adminApi.apps(), []);
  const departments = useAsync(() => adminApi.departments(), []);

  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [department, setDepartment] = useState("");
  const [role, setRole] = useState<Role>("user");
  const [isActive, setIsActive] = useState(true);

  const [selectedApps, setSelectedApps] = useState<Set<string>>(new Set());

  const [tab, setTab] = useState<"profile" | "access">("profile");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmDeactivate, setConfirmDeactivate] = useState(false);
  const [confirmPurge, setConfirmPurge] = useState(false);

  useEffect(() => {
    const record = detail.data;
    if (!record) return;
    setName(record.name);
    setEmail(record.email);
    setDepartment(record.department ?? "");
    setRole(record.role);
    setIsActive(record.isActive);
    setSelectedApps(new Set(detail.data?.appIds ?? []));
  }, [detail.data]);

  const creating = !effectiveId;
  const self = Boolean(effectiveId && me?.id === effectiveId);

  function savedAndClose(message: string) {
    showToast(message);
    window.setTimeout(onClose, 200);
  }

  async function saveProfile() {
    setBusy(true);
    setError(null);

    try {
      if (creating) {
        const input: UserInput = {
          email: email.trim(),
          name: name.trim(),
          department: department.trim() ? department.trim() : null,
          role,
          adminScope: null,
        };
        const created = await adminApi.createUser(input);
        onChanged(created);
        savedAndClose("Saved");
      } else {
        const patch: UserPatch = {
          name: name.trim(),
          department: department.trim() ? department.trim() : null,
          role,
          adminScope: null,
          isActive,
        };
        const updated = await adminApi.updateUser(effectiveId!, patch);
        onChanged(updated);
        savedAndClose("Saved");
      }
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  }

  async function saveAccess() {
    if (!effectiveId) return;
    setBusy(true);
    setError(null);
    try {
      const updated = await adminApi.setUserApps(effectiveId, [...selectedApps]);
      onChanged(updated);
      savedAndClose("Saved");
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  }

  async function deactivate() {
    if (!effectiveId) return;
    setBusy(true);
    setError(null);
    try {
      await adminApi.deactivateUser(effectiveId);
      const record = detail.data;
      if (record) onChanged({ ...record, isActive: false });
      onClose();
    } catch (cause) {
      setError(errorMessage(cause));
      setBusy(false);
      setConfirmDeactivate(false);
    }
  }

  async function purge() {
    if (!effectiveId) return;
    setBusy(true);
    setError(null);
    try {
      await adminApi.purgeUser(effectiveId);
      showToast("User deleted");
      onDeleted?.();
      window.setTimeout(onClose, 200);
    } catch (cause) {
      setError(errorMessage(cause));
      setBusy(false);
      setConfirmPurge(false);
    }
  }

  return (
    <>
      <Drawer
        title={creating ? "New account" : (detail.data?.name ?? "Account")}
        subtitle={
          creating
            ? "Accounts are managed here; no directory service is involved."
            : `${detail.data?.email ?? ""}${detail.data?.department ? ` · ${detail.data.department}` : ""}`
        }
        size="wide"
        onClose={onClose}
        footer={
          <>
            {!creating ? (
              <div className="admin-actions">
                <Button
                  variant="danger"
                  onClick={() => setConfirmPurge(true)}
                  disabled={busy || self}
                  title={self ? "You cannot delete your own account" : undefined}
                >
                  Delete user
                </Button>
                <Button
                  variant="danger"
                  onClick={() => setConfirmDeactivate(true)}
                  disabled={busy || self || !isActive}
                  title={self ? "You cannot deactivate your own account" : undefined}
                >
                  {isActive ? "Deactivate" : "Deactivated"}
                </Button>
              </div>
            ) : (
              <span />
            )}
            <div className="admin-actions">
              <Button variant="ghost" onClick={onClose} disabled={busy}>
                Close
              </Button>
              {tab === "profile" ? (
                <Button
                  variant="primary"
                  onClick={saveProfile}
                  disabled={busy || (!creating && !detail.data)}
                >
                  {busy ? "Saving…" : creating ? "Create account" : "Save changes"}
                </Button>
              ) : (
                <Button variant="primary" onClick={saveAccess} disabled={busy}>
                  {busy ? "Saving…" : "Save access"}
                </Button>
              )}
            </div>
          </>
        }
      >
        {error ? <Notice tone="danger">{error}</Notice> : null}
  
        {!creating && detail.data ? (
          <div className="admin-chiprow">
            <Badge tone={detail.data.isActive ? "success" : "danger"}>
              {detail.data.isActive ? "Active" : "Deactivated"}
            </Badge>
            <Badge tone="accent">{ROLE_LABEL[detail.data.role]}</Badge>
            {detail.data.mustChangePassword ? (
              <Badge tone="warning">Must change password</Badge>
            ) : null}
          </div>
        ) : null}

        {!creating ? (
          <div className="admin-tabs" role="tablist">
            <button
              type="button"
              role="tab"
              className="admin-tab"
              aria-selected={tab === "profile"}
              onClick={() => setTab("profile")}
            >
              Profile & role
            </button>
            <button
              type="button"
              role="tab"
              className="admin-tab"
              aria-selected={tab === "access"}
              onClick={() => setTab("access")}
            >
              Direct app grants
            </button>
          </div>
        ) : null}

        {tab === "profile" ? (
          <div className="admin-form-stack">
            {!creating && detail.loading && !detail.data ? (
              <Spinner label="Loading account…" />
            ) : null}

            <div className="admin-form-grid">
              <TextField
                label="Full name"
                value={name}
                onChange={(event) => setName(event.target.value)}
                maxLength={120}
                required
              />
              <TextField
                label="Email"
                type="email"
                value={email}
                onChange={(event) => setEmail(event.target.value)}
                maxLength={200}
                disabled={!creating}
                hint={creating ? "Used to sign in." : "Email cannot be changed."}
                required
              />
            </div>

            <div className="admin-form-grid">
              <SelectField
                label="Department"
                value={department}
                onChange={(event) => setDepartment(event.target.value)}
                hint="Apps granted to this department appear for the user automatically."
              >
                <option value="">No department</option>
                {(departments.data ?? []).map((item) => (
                  <option key={item.id} value={item.name}>{item.name}</option>
                ))}
              </SelectField>

              <SelectField
                label="Role"
                value={role}
                onChange={(event) => setRole(event.target.value as Role)}
                disabled={self}
                hint={self ? "You cannot change your own role." : undefined}
              >
                <option value="user">User</option>
                <option value="super_admin">Super admin</option>
              </SelectField>
            </div>

            {role === "super_admin" && !self ? (
              <Notice>
                Super admins manage every account and setting. Grant this role sparingly.
              </Notice>
            ) : null}

            {!creating ? (
              <CheckboxField
                label="Account active"
                hint="Deactivating blocks sign-in and removes every app from their client on the next sync."
                checked={isActive}
                disabled={self}
                onChange={setIsActive}
              />
            ) : null}

          </div>
        ) : null}

        {tab === "access" && effectiveId ? (
          <div className="admin-form-stack">
            <div className="admin-stack-sm">
              <h3 className="admin-section-title">Direct app grants</h3>
              <p className="admin-inline-hint">
                Ticked apps are granted directly to this person, in addition to any department
                grants.
              </p>

              {apps.loading && !apps.data ? <Spinner label="Loading apps…" /> : null}

              <div className="admin-checklist">
                {apps.data && apps.data.length === 0 ? (
                  <div className="admin-empty-inline">No apps exist yet.</div>
                ) : null}
                {(apps.data ?? []).map((app) => (
                  <CheckboxField
                    key={app.id}
                    label={app.name}
                    hint={app.category ?? "Uncategorised"}
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

              <p className="admin-inline-hint">
                {selectedApps.size} direct {selectedApps.size === 1 ? "grant" : "grants"} selected.
              </p>
            </div>
          </div>
        ) : null}
      </Drawer>

      <ConfirmDialog
        open={confirmDeactivate}
        title={`Deactivate ${detail.data?.name ?? "this account"}?`}
        description="They are signed out of the desktop client and lose access to every app on the next sync. The account and audit history are kept."
        confirmLabel="Deactivate account"
        busy={busy}
        onCancel={() => setConfirmDeactivate(false)}
        onConfirm={deactivate}
      />

      <ConfirmDialog
        open={confirmPurge}
        title={`Permanently delete ${detail.data?.name ?? "this account"}?`}
        description="This removes the account and all of its direct app grants. A copy of the record is archived in delete_agent.csv for audit. This cannot be undone from the portal."
        confirmLabel="Delete permanently"
        busy={busy}
        onCancel={() => setConfirmPurge(false)}
        onConfirm={purge}
      />
    </>
  );
}
