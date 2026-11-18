import { useEffect, useMemo, useState } from "react";
import { Badge, Button, Spinner } from "@app-directory/ui";
import type {
  AdminApp,
  AdminGroup,
  AdminUser,
  Role,
  UserDetail,
} from "@app-directory/types";
import { adminApi, type UserInput, type UserPatch } from "../api/admin";
import { errorMessage } from "../api/errors";
import { useAuth } from "../auth";
import { ConfirmDialog } from "./ConfirmDialog";
import { Drawer } from "./Drawer";
import { CheckboxField, SelectField, TextField } from "./Form";
import { Notice } from "./PageHeader";
import { useAsync } from "../hooks/useAsync";

interface UserEditorProps {
  userId: string | null;
  onClose: () => void;
  onChanged: (user: AdminUser) => void;
}

const ROLE_LABEL: Record<Role, string> = {
  user: "User",
  app_admin: "App admin",
  super_admin: "Super admin",
};

function parseCategories(text: string): string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const raw of text.split(",")) {
    const value = raw.trim();
    if (!value) continue;
    const key = value.toLowerCase();
    if (seen.has(key)) continue;
    seen.add(key);
    result.push(value.slice(0, 80));
  }
  return result.slice(0, 50);
}

export function UserEditor({ userId, onClose, onChanged }: UserEditorProps) {
  const { user: me } = useAuth();

  const [createdId, setCreatedId] = useState<string | null>(null);
  const effectiveId = createdId ?? userId;

  const detail = useAsync<UserDetail | null>(
    () => (effectiveId ? adminApi.user(effectiveId) : Promise.resolve(null)),
    [effectiveId],
  );

  const apps = useAsync(() => adminApi.apps(), []);
  const groups = useAsync(() => adminApi.groups(), []);
  const departments = useAsync(() => adminApi.departments(), []);

  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [department, setDepartment] = useState("");
  const [role, setRole] = useState<Role>("user");
  const [scopeText, setScopeText] = useState("");
  const [isActive, setIsActive] = useState(true);

  const [selectedApps, setSelectedApps] = useState<Set<string>>(new Set());
  const [groupIds, setGroupIds] = useState<string[]>([]);

  const [tab, setTab] = useState<"profile" | "access">("profile");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [confirmDeactivate, setConfirmDeactivate] = useState(false);

  useEffect(() => {
    const record = detail.data;
    if (!record) return;
    setName(record.name);
    setEmail(record.email);
    setDepartment(record.department ?? "");
    setRole(record.role);
    setScopeText((record.adminScope?.categories ?? []).join(", "));
    setIsActive(record.isActive);
    setSelectedApps(new Set(detail.data?.appIds ?? []));
    setGroupIds(detail.data?.groupIds ?? []);
  }, [detail.data]);

  const creating = !effectiveId;
  const self = Boolean(effectiveId && me?.id === effectiveId);

  const suggestions = useMemo(() => {
    const appList = apps.data ?? [];
    if (groupIds.length === 0) return [] as AdminApp[];
    return appList.filter((app) => app.groupIds.some((id) => groupIds.includes(id)));
  }, [apps.data, groupIds]);

  const pendingSuggestions = suggestions.filter((app) => !selectedApps.has(app.id));

  function applySuggestions() {
    setSelectedApps((previous) => {
      const next = new Set(previous);
      for (const app of suggestions) next.add(app.id);
      return next;
    });
  }

  async function saveProfile() {
    setBusy(true);
    setError(null);
    setNotice(null);

    const scope = role === "app_admin" ? { categories: parseCategories(scopeText) } : null;

    try {
      if (creating) {
        const input: UserInput = {
          email: email.trim(),
          name: name.trim(),
          department: department.trim() ? department.trim() : null,
          role,
          adminScope: scope,
        };
        const created = await adminApi.createUser(input);
        onChanged(created);
        setCreatedId(created.id);
        setNotice("Account created.");
      } else {
        const patch: UserPatch = {
          name: name.trim(),
          department: department.trim() ? department.trim() : null,
          role,
          adminScope: scope,
          isActive,
        };
        const updated = await adminApi.updateUser(effectiveId!, patch);
        onChanged(updated);
        setNotice("Account updated.");
        detail.reload();
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
    setNotice(null);
    try {
      const updated = await adminApi.setUserApps(effectiveId, [...selectedApps]);
      onChanged(updated);
      setGroupIds(updated.groupIds);
      setSelectedApps(new Set(updated.appIds));
      setNotice("Direct app grants saved.");
      detail.reload();
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

  const groupName = (id: string) =>
    groups.data?.find((group) => group.id === id)?.name ?? "Unknown group";

  const groupsById = new Map<string, AdminGroup>(
    (groups.data ?? []).map((group) => [group.id, group]),
  );

  const appSuggestionNote = (app: AdminApp): string | null => {
    const via = app.groupIds.filter((id) => groupIds.includes(id));
    if (via.length === 0) return null;
    return `via ${via.map((id) => groupsById.get(id)?.name ?? groupName(id)).join(", ")}`;
  };

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
              <Button
                variant="danger"
                onClick={() => setConfirmDeactivate(true)}
                disabled={busy || self || !isActive}
                title={self ? "You cannot deactivate your own account" : undefined}
              >
                {isActive ? "Deactivate" : "Deactivated"}
              </Button>
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
        {notice ? <Notice tone="success">{notice}</Notice> : null}

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
                <option value="app_admin">App admin</option>
                <option value="super_admin">Super admin</option>
              </SelectField>
            </div>

            {role === "app_admin" ? (
              <TextField
                label="Admin scope categories"
                value={scopeText}
                onChange={(event) => setScopeText(event.target.value)}
                placeholder="Finance, HR"
                hint="Comma-separated app categories. Empty scope means the admin can see no apps."
              />
            ) : null}

            {role === "super_admin" && !self ? (
              <Notice>
                Super admins manage every account, group and setting. Grant this role sparingly.
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

            {!creating ? (
              <>
                <hr className="admin-divider" />
                <div className="admin-stack-sm">
                  <h3 className="admin-section-title">Group membership</h3>
                  {groupIds.length === 0 ? (
                    <p className="admin-inline-hint">
                      No manual groups include this person. Department groups apply automatically.
                    </p>
                  ) : (
                    <div className="admin-chiprow">
                      {groupIds.map((id) => (
                        <span className="admin-chip" key={id}>
                          {groupName(id)}
                        </span>
                      ))}
                    </div>
                  )}
                  <p className="admin-inline-hint">
                    Edit membership on the Groups screen. Direct grants below add to group access.
                  </p>
                </div>
              </>
            ) : null}
          </div>
        ) : null}

        {tab === "access" && effectiveId ? (
          <div className="admin-form-stack">
            <div className="admin-stack-sm">
              <h3 className="admin-section-title">Direct app grants</h3>
              <p className="admin-inline-hint">
                Ticked apps are granted to this person regardless of group membership. Group access
                is shown underneath each app.
              </p>

              {groups.data && groupIds.length > 0 ? (
                <div className="admin-actions">
                  <Button
                    size="sm"
                    onClick={applySuggestions}
                    disabled={busy || pendingSuggestions.length === 0}
                  >
                    Apply group suggestions ({pendingSuggestions.length})
                  </Button>
                </div>
              ) : null}

              {apps.loading && !apps.data ? <Spinner label="Loading apps…" /> : null}

              <div className="admin-checklist">
                {apps.data && apps.data.length === 0 ? (
                  <div className="admin-empty-inline">No apps exist yet.</div>
                ) : null}
                {(apps.data ?? []).map((app) => {
                  const note = appSuggestionNote(app);
                  return (
                    <CheckboxField
                      key={app.id}
                      label={app.name}
                      hint={
                        note
                          ? `${app.category ?? "Uncategorised"} · ${note}`
                          : (app.category ?? "Uncategorised")
                      }
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
                  );
                })}
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
    </>
  );
}
