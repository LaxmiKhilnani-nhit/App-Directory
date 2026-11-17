import { useEffect, useState } from "react";
import { Button, SearchInput, Spinner } from "@app-directory/ui";
import type { AdminGroup, AdminUser, GroupDetail } from "@app-directory/types";
import { adminApi, type GroupInput } from "../api/admin";
import { errorMessage } from "../api/errors";
import { ConfirmDialog } from "./ConfirmDialog";
import { Drawer } from "./Drawer";
import { TextAreaField, TextField } from "./Form";
import { Notice } from "./PageHeader";
import { useAsync } from "../hooks/useAsync";

interface GroupEditorProps {
  group: AdminGroup | null;
  onClose: () => void;
  onChanged: (group: AdminGroup | null) => void;
}

export function GroupEditor({ group, onClose, onChanged }: GroupEditorProps) {
  const [current, setCurrent] = useState<AdminGroup | null>(group);

  const [name, setName] = useState(group?.name ?? "");
  const [description, setDescription] = useState(group?.description ?? "");
  const [autoDepartment, setAutoDepartment] = useState(group?.autoDepartment ?? "");

  const [members, setMembers] = useState<Set<string>>(new Set());
  const [memberQuery, setMemberQuery] = useState("");

  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const isAllUsers = current?.isAllUsers ?? false;
  const isDerived = isAllUsers || Boolean(current?.autoDepartment);

  const detail = useAsync<GroupDetail | null>(
    () => (current ? adminApi.group(current.id) : Promise.resolve(null)),
    [current?.id],
  );

  const users = useAsync(() => adminApi.users({ limit: 500 }), []);

  useEffect(() => {
    if (detail.data) setMembers(new Set(detail.data.userIds));
  }, [detail.data]);

  function adopt(updated: AdminGroup) {
    setCurrent(updated);
    setName(updated.name);
    setDescription(updated.description ?? "");
    setAutoDepartment(updated.autoDepartment ?? "");
    onChanged(updated);
  }

  async function saveDetails() {
    setBusy(true);
    setError(null);
    setNotice(null);

    const payload: GroupInput = {
      name: name.trim(),
      description: description.trim() ? description.trim() : null,
      autoDepartment: autoDepartment.trim() ? autoDepartment.trim() : null,
    };

    const creating = !current;
    try {
      const saved = creating
        ? await adminApi.createGroup(payload)
        : await adminApi.updateGroup(current!.id, payload);

      adopt(saved);
      setNotice(
        creating
          ? saved.autoDepartment
            ? "Group created. Membership follows the department automatically."
            : "Group created. Add members below."
          : "Group details saved.",
      );
      if (!creating) detail.reload();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  }

  async function saveMembers() {
    if (!current) return;
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      const updated = await adminApi.setGroupMembers(current.id, [...members]);
      setMembers(new Set(updated.userIds));
      adopt(updated);
      setNotice("Membership saved.");
      detail.reload();
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
      await adminApi.deleteGroup(current.id);
      onChanged(null);
      onClose();
    } catch (cause) {
      setError(errorMessage(cause));
      setBusy(false);
      setConfirmDelete(false);
    }
  }

  const selectedRows = [...members]
    .map((id) => users.data?.users.find((person) => person.id === id))
    .filter((person): person is AdminUser => Boolean(person));

  const needle = memberQuery.trim().toLowerCase();
  const candidates =
    users.data?.users
      .filter((person) => person.isActive)
      .filter((person) => !members.has(person.id))
      .filter((person) =>
        !needle
          ? true
          : person.name.toLowerCase().includes(needle) ||
            person.email.toLowerCase().includes(needle) ||
            (person.department ?? "").toLowerCase().includes(needle),
      )
      .slice(0, 12) ?? [];

  return (
    <>
      <Drawer
        title={current ? current.name : "New group"}
        subtitle={
          current
            ? isAllUsers
              ? "Built-in group — every active account belongs to it."
              : current.autoDepartment
                ? `Automatic membership for department "${current.autoDepartment}".`
                : "Manual membership."
            : "Groups grant app access to many people at once."
        }
        size="wide"
        onClose={onClose}
        footer={
          <>
            {current && !isAllUsers ? (
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
              <Button variant="primary" onClick={saveDetails} disabled={busy}>
                {busy ? "Saving…" : current ? "Save details" : "Create group"}
              </Button>
            </div>
          </>
        }
      >
        {error ? <Notice tone="danger">{error}</Notice> : null}
        {notice ? <Notice tone="success">{notice}</Notice> : null}

        <div className="admin-form-stack">
          <TextField
            label="Name"
            value={name}
            onChange={(event) => setName(event.target.value)}
            maxLength={80}
            disabled={isAllUsers}
            required
          />

          <TextAreaField
            label="Description"
            value={description}
            onChange={(event) => setDescription(event.target.value)}
            maxLength={300}
            placeholder="What this group is for"
          />

          <TextField
            label="Auto-include department"
            value={autoDepartment}
            onChange={(event) => setAutoDepartment(event.target.value)}
            maxLength={120}
            disabled={isAllUsers}
            placeholder="Leave blank for a manual group"
            hint={
              isAllUsers
                ? "The built-in group always spans every active account."
                : "When set, membership follows this department and the manual member list is cleared."
            }
          />

          {current?.autoDepartment ? (
            <Notice>
              This group currently follows department "{current.autoDepartment}". Clear the field
              and save to turn it into a manual group — nobody remains a member until you add
              people.
            </Notice>
          ) : null}

          <hr className="admin-divider" />

          <div className="admin-stack-sm">
            <h3 className="admin-section-title">Members</h3>

            {!current ? (
              <p className="admin-inline-hint">Create the group to start adding members.</p>
            ) : isDerived ? (
              <p className="admin-inline-hint">
                {isAllUsers
                  ? "Membership is automatic: every active account is included."
                  : `Membership is automatic: ${current.memberCount} ${
                      current.memberCount === 1 ? "person matches" : "people match"
                    } department "${current.autoDepartment}".`}
              </p>
            ) : (
              <>
                <p className="admin-inline-hint">
                  {current.memberCount} {current.memberCount === 1 ? "member" : "members"}.
                  Membership adds to any direct access a person already has.
                </p>

                {detail.loading && !detail.data ? <Spinner label="Loading members…" /> : null}

                <div className="admin-chiprow">
                  {selectedRows.map((person) => (
                    <span className="admin-chip" key={person.id}>
                      {person.name}
                      <button
                        type="button"
                        aria-label={`Remove ${person.name}`}
                        onClick={() =>
                          setMembers((previous) => {
                            const next = new Set(previous);
                            next.delete(person.id);
                            return next;
                          })
                        }
                      >
                        ×
                      </button>
                    </span>
                  ))}
                  {members.size === 0 ? (
                    <span className="admin-inline-hint">No explicit members yet.</span>
                  ) : null}
                </div>

                <SearchInput
                  value={memberQuery}
                  onValueChange={setMemberQuery}
                  placeholder="Search people by name, email or department…"
                />

                {users.loading && !users.data ? <Spinner label="Loading people…" /> : null}

                {candidates.length > 0 ? (
                  <div className="admin-checklist">
                    {candidates.map((person) => (
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
                          onClick={() => setMembers((previous) => new Set(previous).add(person.id))}
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
                  <Button variant="primary" size="sm" onClick={saveMembers} disabled={busy}>
                    Save membership
                  </Button>
                </div>
              </>
            )}
          </div>

          {current ? (
            <>
              <hr className="admin-divider" />
              <div className="admin-inline-hint">
                {current.appCount} {current.appCount === 1 ? "app grants" : "apps grant"} access to
                this group.
              </div>
            </>
          ) : null}
        </div>
      </Drawer>

      <ConfirmDialog
        open={confirmDelete}
        title={`Delete ${current?.name ?? "this group"}?`}
        description="The group stops granting access immediately. Its members and any app grants stay recorded in the audit log."
        confirmLabel="Delete group"
        busy={busy}
        onCancel={() => setConfirmDelete(false)}
        onConfirm={remove}
      />
    </>
  );
}
