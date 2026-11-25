import { useMemo, useState } from "react";
import { Button, EmptyState, SearchInput, Spinner } from "@app-directory/ui";
import {
  adminApi,
  type PhoneEntry,
  type PhoneEntryInput,
  type PhoneImportReport,
} from "../api/admin";
import { errorMessage } from "../api/errors";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { Drawer } from "../components/Drawer";
import { TextAreaField, TextField } from "../components/Form";
import { Notice, PageHeader } from "../components/PageHeader";
import { showToast } from "../components/Toast";
import { useAsync } from "../hooks/useAsync";

function display(value: string): string {
  return value.trim() ? value : "—";
}

interface PhoneEntryDrawerProps {
  entry: PhoneEntry | null;
  onClose: () => void;
  onSaved: () => void;
}

function PhoneEntryDrawer({ entry, onClose, onSaved }: PhoneEntryDrawerProps) {
  const [firstName, setFirstName] = useState(entry?.firstName ?? "");
  const [lastName, setLastName] = useState(entry?.lastName ?? "");
  const [email, setEmail] = useState(entry?.email ?? "");
  const [phoneNo, setPhoneNo] = useState(entry?.phoneNo ?? "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const canSave =
    !busy &&
    Boolean(firstName.trim()) &&
    Boolean(lastName.trim()) &&
    Boolean(email.trim()) &&
    Boolean(phoneNo.trim());

  async function save() {
    setBusy(true);
    setError(null);
    const input: PhoneEntryInput = {
      firstName: firstName.trim(),
      lastName: lastName.trim(),
      email: email.trim(),
      phoneNo: phoneNo.trim(),
    };

    try {
      if (entry) await adminApi.updatePhoneEntry(entry.id, input);
      else await adminApi.createPhoneEntry(input);
      showToast("Saved");
      onSaved();
      window.setTimeout(onClose, 200);
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Drawer
      title={entry ? "Edit entry" : "Add entry"}
      subtitle="Name, email and phone number for the directory."
      onClose={onClose}
      footer={
        <div className="admin-actions">
          <span />
          <Button variant="ghost" onClick={onClose} disabled={busy}>
            Cancel
          </Button>
          <Button variant="primary" onClick={save} disabled={!canSave}>
            {busy ? "Saving…" : "Save"}
          </Button>
        </div>
      }
    >
      {error ? <Notice tone="danger">{error}</Notice> : null}
      <div className="admin-form-stack">
        <div className="admin-form-grid">
          <TextField
            label="First Name"
            value={firstName}
            onChange={(event) => setFirstName(event.target.value)}
            maxLength={120}
            required
          />
          <TextField
            label="Last Name"
            value={lastName}
            onChange={(event) => setLastName(event.target.value)}
            maxLength={120}
            required
          />
        </div>
        <div className="admin-form-grid">
          <TextField
            label="Email"
            type="email"
            value={email}
            onChange={(event) => setEmail(event.target.value)}
            maxLength={200}
            required
          />
          <TextField
            label="Phone No"
            value={phoneNo}
            onChange={(event) => setPhoneNo(event.target.value)}
            maxLength={60}
            required
          />
        </div>
      </div>
    </Drawer>
  );
}

interface ImportDrawerProps {
  onClose: () => void;
  onImported: () => void;
}

function ImportDrawer({ onClose, onImported }: ImportDrawerProps) {
  const [text, setText] = useState("");
  const [report, setReport] = useState<PhoneImportReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function importRows() {
    setBusy(true);
    setError(null);
    setReport(null);
    try {
      const result = await adminApi.importPhoneDirectory(text);
      setReport(result);
      onImported();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Drawer
      title="Bulk import"
      subtitle="Paste tab- or comma-separated rows from a spreadsheet."
      size="wide"
      onClose={onClose}
      footer={
        <div className="admin-actions">
          <span />
          <Button variant="ghost" onClick={onClose} disabled={busy}>
            Close
          </Button>
          <Button variant="primary" onClick={importRows} disabled={!text.trim() || busy}>
            {busy ? "Importing…" : "Import rows"}
          </Button>
        </div>
      }
    >
      <div className="admin-form-stack">
        {error ? <Notice tone="danger">{error}</Notice> : null}

        <div className="admin-stack-sm">
          <p className="admin-inline-hint">
            Columns: <code>First Name, Last Name, Email Address, Phone No</code>. The header row is
            optional; rows with errors are skipped.
          </p>
          <TextAreaField
            label="Phone directory data"
            className="admin-mono"
            style={{ minHeight: 260 }}
            rows={16}
            value={text}
            onChange={(event) => setText(event.target.value)}
            placeholder={"First Name\tLast Name\tEmail Address\tPhone No\nAsha\tSharma\tasha@example.com\t0123456789"}
            aria-label="Pasted phone directory rows"
          />
        </div>

        {report ? (
          <Notice tone={report.errors.length > 0 ? "info" : "success"}>
            <div>
              Created {report.created}, updated {report.updated}, skipped {report.errors.length}
            </div>
            {report.errors.length > 0 ? (
              <ul className="admin-phone-errors">
                {report.errors.slice(0, 5).map((item, index) => (
                  <li key={`${item.row}-${index}`}>
                    Row {item.row}: {item.message}
                  </li>
                ))}
                {report.errors.length > 5 ? <li>…and {report.errors.length - 5} more.</li> : null}
              </ul>
            ) : null}
          </Notice>
        ) : null}
      </div>
    </Drawer>
  );
}

export function PhoneDirectory() {
  const [query, setQuery] = useState("");
  const [editor, setEditor] = useState<{ entry: PhoneEntry | null } | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<PhoneEntry | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const [importOpen, setImportOpen] = useState(false);

  const directory = useAsync(() => adminApi.phoneDirectory(), []);

  const rows = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return directory.data ?? [];
    return (directory.data ?? []).filter((entry) =>
      [entry.firstName, entry.lastName, `${entry.firstName} ${entry.lastName}`, entry.email, entry.phoneNo]
        .join(" ")
        .toLowerCase()
        .includes(needle),
    );
  }, [directory.data, query]);

  async function confirmDelete() {
    if (!deleteTarget) return;
    setDeleting(true);
    setDeleteError(null);
    try {
      await adminApi.deletePhoneEntry(deleteTarget.id);
      showToast("Deleted");
      setDeleteTarget(null);
      directory.reload();
    } catch (cause) {
      setDeleteError(errorMessage(cause));
    } finally {
      setDeleting(false);
    }
  }

  return (
    <>
      <PageHeader
        title="Phone Directory"
        description="Names, email addresses and phone numbers for the organisation."
        actions={
          <>
            <Button onClick={() => setImportOpen(true)}>Bulk import</Button>
            <Button variant="primary" onClick={() => setEditor({ entry: null })}>
              Add entry
            </Button>
          </>
        }
      />

      {directory.error ? <Notice tone="danger">{directory.error}</Notice> : null}

      <section className="admin-card">
        <div className="admin-toolbar">
          <SearchInput
            value={query}
            onValueChange={setQuery}
            placeholder="Search by name, email or phone no…"
          />
        </div>

        {directory.loading && !directory.data ? (
          <Spinner label="Loading phone directory…" />
        ) : rows.length === 0 ? (
          <EmptyState
            title={directory.data && directory.data.length > 0 ? "No entries match your search" : "No entries yet"}
            description={
              directory.data && directory.data.length > 0
                ? "Try a broader search."
                : "Add an entry one at a time, or bulk import pasted rows."
            }
          />
        ) : (
          <div className="admin-table-wrap" style={{ marginTop: "var(--space-3)" }}>
            <table className="admin-table admin-phone-table">
              <thead>
                <tr>
                  <th>First Name</th>
                  <th>Last Name</th>
                  <th>Email</th>
                  <th>Phone No</th>
                  <th aria-label="Actions" />
                </tr>
              </thead>
              <tbody>
                {rows.map((entry) => (
                  <tr key={entry.id}>
                    <td>{display(entry.firstName)}</td>
                    <td>{display(entry.lastName)}</td>
                    <td className="admin-mono">{display(entry.email)}</td>
                    <td className="admin-mono admin-nowrap">{display(entry.phoneNo)}</td>
                    <td>
                      <div className="admin-table__actions">
                        <Button size="sm" onClick={() => setEditor({ entry })}>
                          Edit
                        </Button>
                        <Button
                          size="sm"
                          variant="danger"
                          onClick={() => {
                            setDeleteError(null);
                            setDeleting(false);
                            setDeleteTarget(entry);
                          }}
                        >
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

        <p className="admin-inline-hint" style={{ marginTop: "var(--space-3)" }}>
          Showing {rows.length} of {directory.data?.length ?? 0} entries
        </p>
      </section>

      {editor ? (
        <PhoneEntryDrawer
          key={editor.entry?.id ?? "new"}
          entry={editor.entry}
          onClose={() => setEditor(null)}
          onSaved={directory.reload}
        />
      ) : null}

      {importOpen ? (
        <ImportDrawer onClose={() => setImportOpen(false)} onImported={directory.reload} />
      ) : null}

      <ConfirmDialog
        open={Boolean(deleteTarget)}
        title="Delete this phone directory entry?"
        description={deleteError ?? "This removes the entry from the directory. This cannot be undone."}
        confirmLabel="Delete entry"
        busy={deleting}
        onCancel={() => setDeleteTarget(null)}
        onConfirm={confirmDelete}
      />
    </>
  );
}
