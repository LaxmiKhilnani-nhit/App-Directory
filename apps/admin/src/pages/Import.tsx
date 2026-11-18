import { useMemo, useState } from "react";
import { Badge, Button, Spinner } from "@app-directory/ui";
import type { ImportReport } from "@app-directory/types";
import { adminApi } from "../api/admin";
import { errorMessage } from "../api/errors";
import { PageHeader, Notice } from "../components/PageHeader";
import { useAsync } from "../hooks/useAsync";

interface ParsedRow {
  line: number;
  name: string;
  email: string;
  department: string;
  groups: string[];
}

interface ParsedCsv {
  columns: string[];
  mapping: { name: string; email: string; department: string | null; groups: string | null };
  rows: ParsedRow[];
  error: string | null;
}

const EMPTY: ParsedCsv = {
  columns: [],
  mapping: { name: "", email: "", department: null, groups: null },
  rows: [],
  error: null,
};

/** Mirrors the server's line splitter so the preview matches what will be stored. */
function splitCsvLine(line: string): string[] {
  const fields: string[] = [];
  let current = "";
  let quoted = false;

  for (let index = 0; index < line.length; index += 1) {
    const character = line[index];
    if (character === '"' && quoted && line[index + 1] === '"') {
      current += '"';
      index += 1;
    } else if (character === '"') {
      quoted = !quoted;
    } else if (character === "," && !quoted) {
      fields.push(current.trim());
      current = "";
    } else {
      current += character;
    }
  }

  fields.push(current.trim());
  return fields;
}

function parseCsv(text: string): ParsedCsv {
  const lines = text
    .split(/\r?\n/)
    .map((raw, index) => ({ line: index + 1, raw }))
    .filter((entry) => entry.raw.trim() !== "");

  if (lines.length === 0) return { ...EMPTY, error: "The file is empty." };

  const columns = splitCsvLine(lines[0]!.raw).map((column) =>
    column.trim().toLowerCase().replace(/ /g, "_"),
  );
  const indexOf = (name: string) => columns.indexOf(name);

  const nameIndex = indexOf("name");
  const emailIndex = indexOf("email");
  if (nameIndex < 0 || emailIndex < 0) {
    return {
      ...EMPTY,
      columns,
      error: "The header must include name and email columns.",
    };
  }

  const departmentIndex = indexOf("department");
  const groupsIndex = indexOf("groups");
  const field = (fields: string[], index: number) => (index >= 0 ? (fields[index] ?? "") : "");

  const rows = lines.slice(1).map(({ line, raw }) => {
    const fields = splitCsvLine(raw);
    return {
      line,
      name: field(fields, nameIndex),
      email: field(fields, emailIndex),
      department: field(fields, departmentIndex),
      groups: field(fields, groupsIndex)
        .split(";")
        .map((group) => group.trim())
        .filter(Boolean),
    };
  });

  return {
    columns,
    mapping: {
      name: columns[nameIndex] ?? "name",
      email: columns[emailIndex] ?? "email",
      department: departmentIndex >= 0 ? (columns[departmentIndex] ?? null) : null,
      groups: groupsIndex >= 0 ? (columns[groupsIndex] ?? null) : null,
    },
    rows,
    error: null,
  };
}

function rowErrors(row: ParsedRow, seen: Set<string>, knownGroups: Set<string> | null): string[] {
  const problems: string[] = [];
  if (!row.name.trim()) problems.push("Name is empty.");
  if (!row.email.trim()) problems.push("Email is empty.");
  else if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(row.email.trim()))
    problems.push("Email is not valid.");

  const key = row.email.trim().toLowerCase();
  if (key) {
    if (seen.has(key)) problems.push("Duplicate email in this file.");
    seen.add(key);
  }

  if (knownGroups) {
    const unknown = row.groups.filter((group) => !knownGroups.has(group.toLowerCase()));
    if (unknown.length > 0) problems.push(`Unknown group: ${unknown.join(", ")}.`);
  }

  return problems;
}


export function Import() {
  const [csv, setCsv] = useState("");
  const [report, setReport] = useState<ImportReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  

  const groups = useAsync(() => adminApi.groups(), []);

  const parsed = useMemo(() => (csv.trim() ? parseCsv(csv) : EMPTY), [csv]);

  const knownGroups = useMemo(() => {
    const names = new Set<string>();
    for (const group of groups.data ?? []) {
      if (!group.isAllUsers && !group.autoDepartment) names.add(group.name.toLowerCase());
    }
    return names;
  }, [groups.data]);

  const checked = useMemo(() => {
    const seen = new Set<string>();
    const groupsReady = groups.data ? knownGroups : null;
    return parsed.rows.map((row) => ({
      row,
      problems: rowErrors(row, seen, groupsReady),
    }));
  }, [parsed.rows, knownGroups, groups.data]);

  const validRows = checked.filter((entry) => entry.problems.length === 0).length;
  const problemRows = checked.filter((entry) => entry.problems.length > 0).length;

  async function onFile(file: File | null) {
    if (!file) return;
    setError(null);
    setReport(null);
    try {
      setCsv(await file.text());
    } catch {
      setError("That file could not be read as text.");
    }
  }

  async function commit() {
    setBusy(true);
    setError(null);
    setReport(null);
    try {
      const result = await adminApi.importUsers(csv);
      setReport(result);
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  }


  const canCommit = Boolean(csv.trim()) && !parsed.error && parsed.rows.length > 0 && !busy;

  return (
    <>
      <PageHeader
        title="CSV import"
        description="Bulk-create or update accounts. Valid rows are imported; problem rows are reported and skipped."
      />

      {error ? <Notice tone="danger">{error}</Notice> : null}
      {groups.error ? <Notice tone="danger">{groups.error}</Notice> : null}

      <section className="admin-card">
        <div className="admin-form-stack">
          <div className="admin-stack-sm">
            <h3 className="admin-section-title">1. Choose a file or paste CSV</h3>
            <p className="admin-inline-hint">
              Header: <code>name,email,department,groups</code>. Separate multiple groups with a
              semicolon. Unknown groups are row errors — create them first.
            </p>
            <div className="admin-actions">
              <label className="ad-btn ad-btn--secondary" style={{ cursor: "pointer" }}>
                Choose CSV file
                <input
                  type="file"
                  accept=".csv,text/csv,text/plain"
                  hidden
                  onChange={(event) => {
                    void onFile(event.target.files?.[0] ?? null);
                    event.target.value = "";
                  }}
                />
              </label>
              <Button
                variant="ghost"
                onClick={() => {
                  setCsv("");
                  setReport(null);
                  setError(null);
                }}
              >
                Clear
              </Button>
            </div>
          </div>

          <textarea
            className="admin-mono"
            style={{ minHeight: 130 }}
            value={csv}
            onChange={(event) => setCsv(event.target.value)}
            placeholder={
              "name,email,department,groups\nRahul Sharma,rahul@example.com,Finance,Finance;Reporting"
            }
            aria-label="CSV contents"
          />

          {parsed.error ? <Notice tone="danger">{parsed.error}</Notice> : null}

          {parsed.columns.length > 0 && !parsed.error ? (
            <div className="admin-stack-sm">
              <h3 className="admin-section-title">2. Column mapping</h3>
              <div className="admin-chiprow">
                <span className="admin-chip">Name ← {parsed.mapping.name}</span>
                <span className="admin-chip">Email ← {parsed.mapping.email}</span>
                <span className="admin-chip">
                  Department ← {parsed.mapping.department ?? "not provided"}
                </span>
                <span className="admin-chip">
                  Groups ← {parsed.mapping.groups ?? "not provided"}
                </span>
              </div>
            </div>
          ) : null}

          {parsed.rows.length > 0 && !parsed.error ? (
            <div className="admin-stack-sm">
              <h3 className="admin-section-title">3. Validate & preview</h3>
              <div className="admin-import-summary">
                <span>
                  <strong>{parsed.rows.length}</strong>
                  rows
                </span>
                <span>
                  <strong style={{ color: "var(--color-success)" }}>{validRows}</strong>
                  ready
                </span>
                <span>
                  <strong style={{ color: problemRows ? "var(--color-danger)" : undefined }}>
                    {problemRows}
                  </strong>
                  with problems
                </span>
              </div>

              {groups.data ? (
                <p className="admin-inline-hint">
                  {knownGroups.size} manual {knownGroups.size === 1 ? "group" : "groups"} can be
                  referenced. Automatic and All Users groups cannot be used in this file.
                </p>
              ) : null}

              <div className="admin-table-wrap" style={{ maxHeight: 360, overflowY: "auto" }}>
                <table className="admin-table">
                  <thead>
                    <tr>
                      <th>Line</th>
                      <th>Name</th>
                      <th>Email</th>
                      <th>Department</th>
                      <th>Groups</th>
                      <th>Check</th>
                    </tr>
                  </thead>
                  <tbody>
                    {checked.map(({ row, problems }) => (
                      <tr key={row.line}>
                        <td className="admin-mono admin-muted-cell">{row.line}</td>
                        <td>{row.name || "—"}</td>
                        <td className="admin-mono">{row.email || "—"}</td>
                        <td className="admin-muted-cell">{row.department || "—"}</td>
                        <td className="admin-muted-cell">
                          {row.groups.length > 0 ? row.groups.join("; ") : "—"}
                        </td>
                        <td>
                          {problems.length === 0 ? (
                            <Badge tone="success">Ready</Badge>
                          ) : (
                            <span style={{ color: "var(--color-danger)", fontSize: 12.5 }}>
                              {problems.join(" ")}
                            </span>
                          )}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>

              <p className="admin-inline-hint">
                The server validates again on commit. New accounts must set a password through the
                existing accounts keep their roles and are updated from the CSV.
              </p>

              <div className="admin-actions">
                <Button variant="primary" onClick={commit} disabled={!canCommit}>
                  {busy ? "Importing…" : `Import ${parsed.rows.length} rows`}
                </Button>
                {busy ? <Spinner label="" /> : null}
              </div>
              {busy ? (
                <p className="admin-inline-hint">
                  Importing {parsed.rows.length} rows. Large files take several seconds — leave this
                  page open until the report appears.
                </p>
              ) : null}
            </div>
          ) : null}
        </div>
      </section>

      {report ? (
        <section className="admin-card">
          <h2 className="admin-card__title">Import result</h2>

          <div className="admin-import-summary" style={{ marginTop: "var(--space-3)" }}>
            <span>
              <strong>{report.created}</strong>
              created
            </span>
            <span>
              <strong>{report.updated}</strong>
              updated
            </span>
            <span>
              <strong>{report.unchanged}</strong>
              unchanged
            </span>
            <span>
              <strong style={{ color: report.errors.length ? "var(--color-danger)" : undefined }}>
                {report.errors.length}
              </strong>
              row errors
            </span>
          </div>

          {report.errors.length > 0 ? (
            <div className="admin-table-wrap" style={{ marginTop: "var(--space-3)" }}>
              <table className="admin-table">
                <thead>
                  <tr>
                    <th>Line</th>
                    <th>Email</th>
                    <th>Problem</th>
                  </tr>
                </thead>
                <tbody>
                  {report.errors.map((row) => (
                    <tr key={`${row.line}-${row.email ?? ""}`}>
                      <td className="admin-mono admin-muted-cell">{row.line}</td>
                      <td className="admin-mono">{row.email ?? "—"}</td>
                      <td>{row.message}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : null}

        </section>
      ) : null}
    </>
  );
}