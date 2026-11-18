import { useState } from "react";
import { Button, SearchInput, Spinner, Tile } from "@app-directory/ui";
import type { AdminApp, AdminGroup, AdminUser, AppTile, Department } from "@app-directory/types";
import { adminApi, type AppInput } from "../api/admin";
import { errorMessage } from "../api/errors";
import { useAuth } from "../auth";
import { ConfirmDialog } from "./ConfirmDialog";
import { Drawer } from "./Drawer";
import { CheckboxField, TextAreaField, TextField } from "./Form";
import { Notice } from "./PageHeader";
import { useAsync } from "../hooks/useAsync";
import { forgetIcon, useIconUrl } from "../hooks/useIconUrl";

interface AppEditorProps {
  app: AdminApp | null;
  groups: AdminGroup[];
  onClose: () => void;
  onChanged: (app: AdminApp | null) => void;
}

type Tab = "details" | "access";

export function AppEditor({ app, groups, onClose, onChanged }: AppEditorProps) {
  const { user } = useAuth();
  const isSuper = user?.role === "super_admin";

  const [current, setCurrent] = useState<AdminApp | null>(app);
  const [tab, setTab] = useState<Tab>("details");

  const [name, setName] = useState(app?.name ?? "");
  const [url, setUrl] = useState(app?.url ?? "");
  const [description, setDescription] = useState(app?.description ?? "");
  const [category, setCategory] = useState(app?.category ?? "");
  const [color, setColor] = useState(app?.color ?? "#2F6FED");
  const [sortOrder, setSortOrder] = useState(String(app?.sortOrder ?? 100));
  const [isEnabled, setIsEnabled] = useState(app?.isEnabled ?? true);
  const [offlineVisible, setOfflineVisible] = useState(app?.offlineVisible ?? true);

  const [selectedDepartments, setSelectedDepartments] = useState<Set<string>>(
    () => new Set(app?.departmentIds ?? []),
  );
  const [selectedGroups, setSelectedGroups] = useState<Set<string>>(
    () => new Set(app?.groupIds ?? []),
  );
  const [selectedUsers, setSelectedUsers] = useState<Set<string>>(
    () => new Set(app?.userIds ?? []),
  );
  const [userQuery, setUserQuery] = useState("");

  const [busy, setBusy] = useState(false);
  const [iconBusy, setIconBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const departments = useAsync(() => adminApi.departments(), []);
  const users = useAsync(
    () => (isSuper ? adminApi.users({ limit: 500 }) : Promise.resolve(null)),
    [isSuper],
  );

  const iconUrl = useIconUrl(current?.id, current?.iconEtag);

  const userName = (id: string) =>
    users.data?.users.find((person) => person.id === id)?.name ?? id.slice(0, 8);

  function resetAccessState(updated: AdminApp) {
    setCurrent(updated);
    setSelectedDepartments(new Set(updated.departmentIds));
    setSelectedGroups(new Set(updated.groupIds));
    setSelectedUsers(new Set(updated.userIds));
    onChanged(updated);
  }

  async function saveDetails() {
    setBusy(true);
    setError(null);
    setNotice(null);

    const payload: AppInput = {
      name: name.trim(),
      url: url.trim(),
      description: description.trim() ? description.trim() : null,
      category: category.trim() ? category.trim() : null,
      color: color.trim() ? color.trim() : null,
      sortOrder: Number(sortOrder) || 0,
      isEnabled,
      offlineVisible,
    };

    const creating = !current;
    try {
      const saved = creating
        ? await adminApi.createApp(payload)
        : await adminApi.updateApp(current!.id, payload);

      resetAccessState(saved);
      setNotice(creating ? "App created. Add group or user access next." : "Changes saved.");
      if (creating) setTab("access");
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  }

  async function onUploadIcon(file: File | null) {
    if (!file || !current) return;
    setIconBusy(true);
    setError(null);
    setNotice(null);
    try {
      const updated = await adminApi.uploadIcon(current.id, file);
      forgetIcon(updated.id);
      resetAccessState(updated);
      setNotice("Icon updated.");
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setIconBusy(false);
    }
  }

  async function refetchIcon() {
    if (!current) return;
    setIconBusy(true);
    setError(null);
    setNotice(null);
    try {
      const updated = await adminApi.refetchIcon(current.id);
      forgetIcon(updated.id);
      resetAccessState(updated);
      setNotice("Favicon fetched from the app's site.");
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setIconBusy(false);
    }
  }

  async function saveGroups() {
    if (!current) return;
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      const updated = await adminApi.setAppGroups(current.id, [...selectedGroups]);
      resetAccessState(updated);
      setNotice("Group access saved.");
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  }

  async function saveUsers() {
    if (!current) return;
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      const updated = await adminApi.setAppUsers(current.id, [...selectedUsers]);
      resetAccessState(updated);
      setNotice("Direct user access saved.");
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    if (!current) return;
    setBusy(true);
    setError(null);
    try {
      await adminApi.deleteApp(current.id);
      onChanged(null);
      onClose();
    } catch (cause) {
      setError(errorMessage(cause));
      setBusy(false);
      setConfirmDelete(false);
    }
  }

  const preview: AppTile = {
    id: current?.id ?? "preview",
    name: name.trim() || "New app",
    url: url.trim(),
    description: description.trim() || null,
    category: category.trim() || null,
    color: color.trim() || null,
    sortOrder: Number(sortOrder) || 0,
    offlineVisible,
    iconUrl,
    iconEtag: current?.iconEtag ?? null,
    updatedAt: current?.updatedAt ?? new Date().toISOString(),
  };

  const filteredUsers =
    users.data?.users
      .filter((person) => !selectedUsers.has(person.id))
      .filter((person) => {
        const needle = userQuery.trim().toLowerCase();
        if (!needle) return true;
        return (
          person.name.toLowerCase().includes(needle) ||
          person.email.toLowerCase().includes(needle) ||
          (person.department ?? "").toLowerCase().includes(needle)
        );
      })
      .slice(0, 12) ?? [];

  return (
    <>
      <Drawer
        title={current ? current.name : "New application"}
        subtitle={current ? current.url : "Add a tile to the directory."}
        size="wide"
        onClose={onClose}
        footer={
          <>
            {current ? (
              <Button variant="danger" onClick={() => setConfirmDelete(true)} disabled={busy}>
                Delete
              </Button>
            ) : (
              <span />
            )}
            <div className="admin-actions">
              <Button variant="ghost" onClick={onClose} disabled={busy}>
                Close
              </Button>
              {tab === "details" ? (
                <Button variant="primary" onClick={saveDetails} disabled={busy}>
                  {busy ? "Saving…" : current ? "Save changes" : "Create app"}
                </Button>
              ) : null}
            </div>
          </>
        }
      >
        {error ? <Notice tone="danger">{error}</Notice> : null}
        {notice ? <Notice tone="success">{notice}</Notice> : null}

        <div className="admin-tabs" role="tablist">
          <button
            type="button"
            role="tab"
            className="admin-tab"
            aria-selected={tab === "details"}
            onClick={() => setTab("details")}
          >
            Details
          </button>
          <button
            type="button"
            role="tab"
            className="admin-tab"
            aria-selected={tab === "access"}
            onClick={() => setTab("access")}
            disabled={!current}
            title={current ? undefined : "Create the app first"}
          >
            Access
          </button>
        </div>

        {tab === "details" ? (
          <div className="admin-form-stack">
            <div className="admin-form-grid">
              <TextField
                label="Name"
                value={name}
                onChange={(event) => setName(event.target.value)}
                maxLength={120}
                required
              />
              <TextField
                label="Category"
                value={category}
                onChange={(event) => setCategory(event.target.value)}
                placeholder="Finance"
                maxLength={80}
              />
            </div>

            <TextField
              label="URL"
              value={url}
              onChange={(event) => setUrl(event.target.value)}
              placeholder="http://172.168.50.50:8080/payroll"
              maxLength={500}
              required
            />

            <TextAreaField
              label="Description"
              value={description}
              onChange={(event) => setDescription(event.target.value)}
              maxLength={500}
              placeholder="Shown under the tile name"
            />

            <div className="admin-form-grid">
              <div className="admin-field">
                <label htmlFor="app-colour">Colour</label>
                <div style={{ display: "flex", gap: "var(--space-2)" }}>
                  <input
                    id="app-colour"
                    type="color"
                    value={/^#[0-9a-fA-F]{6}$/.test(color) ? color : "#2F6FED"}
                    onChange={(event) => setColor(event.target.value.toUpperCase())}
                    style={{ width: 44, padding: 3 }}
                  />
                  <input
                    className="admin-mono"
                    value={color}
                    onChange={(event) => setColor(event.target.value)}
                    placeholder="#2F6FED"
                    maxLength={7}
                    aria-label="Colour hex value"
                  />
                </div>
              </div>

              <TextField
                label="Sort order"
                type="number"
                min={-10000}
                max={10000}
                value={sortOrder}
                onChange={(event) => setSortOrder(event.target.value)}
                hint="Lower numbers appear first."
              />
            </div>

            <div className="admin-stack-sm">
              <CheckboxField
                label="Enabled"
                hint="Disabled apps disappear from every client on the next sync."
                checked={isEnabled}
                onChange={setIsEnabled}
              />
              <CheckboxField
                label="Available offline"
                hint="When off, this app is never written to a client's encrypted snapshot."
                checked={offlineVisible}
                onChange={setOfflineVisible}
              />
            </div>

            <hr className="admin-divider" />

            <div className="admin-preview">
              <Tile app={preview} onOpen={() => undefined} />
            </div>

            {current ? (
              <div className="admin-stack-sm">
                <h3 className="admin-section-title">Icon</h3>
                <div className="admin-inline-hint">
                  {current.iconEtag
                    ? `Stored from ${current.iconSource === "favicon" ? "the app's favicon" : "an upload"}.`
                    : "No icon yet — tiles show a coloured letter mark."}
                </div>
                <div className="admin-actions">
                  <label
                    className="ad-btn ad-btn--secondary ad-btn--sm"
                    style={{ cursor: "pointer" }}
                  >
                    {iconBusy ? "Working…" : "Upload icon"}
                    <input
                      type="file"
                      accept="image/png,image/jpeg,image/x-icon,image/gif,image/webp,image/svg+xml"
                      hidden
                      disabled={iconBusy}
                      onChange={(event) => {
                        void onUploadIcon(event.target.files?.[0] ?? null);
                        event.target.value = "";
                      }}
                    />
                  </label>
                  <Button size="sm" onClick={refetchIcon} disabled={iconBusy}>
                    Refetch favicon
                  </Button>
                  {iconBusy ? <Spinner label="" /> : null}
                </div>
              </div>
            ) : null}
          </div>
        ) : null}

        {tab === "access" && current ? (
          <div className="admin-form-stack">
            <div className="admin-stack-sm">
              <h3 className="admin-section-title">Departments with access</h3>
              <p className="admin-inline-hint">
                Everyone assigned to a selected department sees this tile. Changes apply on the
                next client sync.
              </p>
              <div className="admin-checklist">
                {!departments.data || departments.data.length === 0 ? (
                  <div className="admin-empty-inline">
                    No departments yet — add them on the Departments screen.
                  </div>
                ) : (
                  departments.data.map((department: Department) => (
                    <CheckboxField
                      key={department.id}
                      label={department.name}
                      hint={`${department.userCount} people · ${department.appCount} apps`}
                      checked={selectedDepartments.has(department.id)}
                      onChange={(checked) => {
                        const next = new Set(selectedDepartments);
                        if (checked) next.add(department.id);
                        else next.delete(department.id);
                        setSelectedDepartments(next);
                        if (!current) return;
                        setBusy(true);
                        adminApi
                          .setAppDepartments(current.id, [...next])
                          .then((updated) => {
                            setCurrent(updated);
                            setNotice("Department access saved.");
                            onChanged(updated);
                          })
                          .catch((cause) => setError(errorMessage(cause)))
                          .finally(() => setBusy(false));
                      }}
                    />
                  ))
                )}
              </div>
              <p className="admin-inline-hint">Changes save automatically.</p>
            </div>

            <hr className="admin-divider" />

            <div className="admin-stack-sm">
              <h3 className="admin-section-title">Groups with access</h3>
              <p className="admin-inline-hint">
                Everyone in a selected group sees this tile: explicit members and the built-in All
                Users group.
              </p>
              <div className="admin-checklist">
                {groups.length === 0 ? (
                  <div className="admin-empty-inline">No groups exist yet.</div>
                ) : (
                  groups.map((group) => (
                    <CheckboxField
                      key={group.id}
                      label={`${group.name}${group.isAllUsers ? " (everyone)" : ""}`}
                      hint={
                        group.autoDepartment
                          ? `Automatic · department ${group.autoDepartment} · ${group.memberCount} members`
                          : `${group.memberCount} members`
                      }
                      checked={selectedGroups.has(group.id)}
                      onChange={(checked) =>
                        setSelectedGroups((previous) => {
                          const next = new Set(previous);
                          if (checked) next.add(group.id);
                          else next.delete(group.id);
                          return next;
                        })
                      }
                    />
                  ))
                )}
              </div>
              <div className="admin-actions">
                <Button variant="primary" size="sm" onClick={saveGroups} disabled={busy}>
                  Save group access
                </Button>
              </div>
            </div>

            <hr className="admin-divider" />

            <div className="admin-stack-sm">
              <h3 className="admin-section-title">Direct user access</h3>
              {!isSuper ? (
                <Notice>
                  Searching accounts is limited to super admins. App admins manage access through
                  groups; direct grants can still be added by a super admin.
                </Notice>
              ) : (
                <>
                  <div className="admin-inline-hint">
                    Direct grants are on top of group access. Removing a group grant here does not
                    take away access a person gets elsewhere.
                  </div>

                  <div className="admin-chiprow">
                    {[...selectedUsers].map((id) => (
                      <span className="admin-chip" key={id}>
                        {userName(id)}
                        <button
                          type="button"
                          aria-label={`Remove ${userName(id)}`}
                          onClick={() =>
                            setSelectedUsers((previous) => {
                              const next = new Set(previous);
                              next.delete(id);
                              return next;
                            })
                          }
                        >
                          ×
                        </button>
                      </span>
                    ))}
                    {selectedUsers.size === 0 ? (
                      <span className="admin-inline-hint">No direct grants.</span>
                    ) : null}
                  </div>

                  <SearchInput
                    value={userQuery}
                    onValueChange={setUserQuery}
                    placeholder="Search people by name, email or department…"
                  />

                  {users.loading && !users.data ? <Spinner label="Loading people…" /> : null}

                  {filteredUsers.length > 0 ? (
                    <div className="admin-checklist">
                      {filteredUsers.map((person: AdminUser) => (
                        <div
                          key={person.id}
                          style={{
                            display: "flex",
                            alignItems: "center",
                            justifyContent: "space-between",
                            gap: "var(--space-3)",
                          }}
                        >
                          <span>
                            <span className="admin-checkbox__label">{person.name}</span>{" "}
                            <span className="admin-muted-cell admin-mono">{person.email}</span>
                          </span>
                          <Button
                            size="sm"
                            onClick={() =>
                              setSelectedUsers((previous) => new Set(previous).add(person.id))
                            }
                          >
                            Add
                          </Button>
                        </div>
                      ))}
                    </div>
                  ) : (
                    <div className="admin-empty-inline">No more matching people.</div>
                  )}

                  <div className="admin-actions">
                    <Button variant="primary" size="sm" onClick={saveUsers} disabled={busy}>
                      Save user access
                    </Button>
                  </div>
                </>
              )}
            </div>
          </div>
        ) : null}
      </Drawer>

      <ConfirmDialog
        open={confirmDelete}
        title={`Delete ${current?.name ?? "this app"}?`}
        description="The tile disappears from every client on the next sync. Grants and audit history are kept, so the app can be restored from the database if needed."
        confirmLabel="Delete app"
        busy={busy}
        onCancel={() => setConfirmDelete(false)}
        onConfirm={remove}
      />
    </>
  );
}
