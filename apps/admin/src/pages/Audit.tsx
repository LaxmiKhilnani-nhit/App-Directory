import { Fragment, useState, type FormEvent } from "react";
import { Badge, Button, EmptyState, Spinner } from "@app-directory/ui";
import { adminApi, type AuditQuery } from "../api/admin";
import { PageHeader, Notice } from "../components/PageHeader";
import { useAsync } from "../hooks/useAsync";
import { formatDateTime } from "../utils/format";

const PAGE_SIZE = 50;

const ENTITY_TYPES = ["app", "group", "user", "settings", "auth"];

function isoStart(value: string): string | undefined {
  if (!value) return undefined;
  const date = new Date(`${value}T00:00:00`);
  return Number.isNaN(date.getTime()) ? undefined : date.toISOString();
}

function isoEnd(value: string): string | undefined {
  if (!value) return undefined;
  const date = new Date(`${value}T23:59:59.999`);
  return Number.isNaN(date.getTime()) ? undefined : date.toISOString();
}

interface AppliedFilters {
  actor: string;
  entityType: string;
  action: string;
  from: string;
  to: string;
}

const EMPTY_FILTERS: AppliedFilters = { actor: "", entityType: "", action: "", from: "", to: "" };

function Json({ value }: { value: unknown }) {
  if (value === null || value === undefined) return <span className="admin-muted-cell">—</span>;
  return <pre className="admin-json">{JSON.stringify(value, null, 2)}</pre>;
}

export function Audit() {
  const [draft, setDraft] = useState<AppliedFilters>(EMPTY_FILTERS);
  const [applied, setApplied] = useState<AppliedFilters>(EMPTY_FILTERS);
  const [offset, setOffset] = useState(0);
  const [expanded, setExpanded] = useState<string | null>(null);

  const audit = useAsync(() => {
    const params: AuditQuery = {
      actor: applied.actor.trim() || undefined,
      entityType: applied.entityType || undefined,
      action: applied.action.trim() || undefined,
      from: isoStart(applied.from),
      to: isoEnd(applied.to),
      limit: PAGE_SIZE,
      offset,
    };
    return adminApi.audit(params);
  }, [applied, offset]);

  function apply(event: FormEvent) {
    event.preventDefault();
    setOffset(0);
    setExpanded(null);
    setApplied(draft);
  }

  function reset() {
    setDraft(EMPTY_FILTERS);
    setApplied(EMPTY_FILTERS);
    setOffset(0);
    setExpanded(null);
  }

  const total = audit.data?.total ?? 0;
  const entries = audit.data?.entries ?? [];
  const start = entries.length > 0 ? offset + 1 : 0;
  const end = offset + entries.length;

  return (
    <>
      <PageHeader
        title="Audit log"
        description="Every admin change, with the acting account, the entity and before/after values."
      />

      {audit.error ? <Notice tone="danger">{audit.error}</Notice> : null}

      <section className="admin-card">
        <form className="admin-filters" onSubmit={apply}>
          <div className="admin-field">
            <label htmlFor="audit-actor">Actor</label>
            <input
              id="audit-actor"
              value={draft.actor}
              onChange={(event) => setDraft({ ...draft, actor: event.target.value })}
              placeholder="email or user id"
            />
          </div>

          <div className="admin-field">
            <label htmlFor="audit-entity">Entity type</label>
            <select
              id="audit-entity"
              value={draft.entityType}
              onChange={(event) => setDraft({ ...draft, entityType: event.target.value })}
            >
              <option value="">All entities</option>
              {ENTITY_TYPES.map((type) => (
                <option key={type} value={type}>
                  {type}
                </option>
              ))}
            </select>
          </div>

          <div className="admin-field">
            <label htmlFor="audit-action">Action</label>
            <input
              id="audit-action"
              value={draft.action}
              onChange={(event) => setDraft({ ...draft, action: event.target.value })}
              placeholder="app.update"
            />
          </div>

          <div className="admin-field">
            <label htmlFor="audit-from">From</label>
            <input
              id="audit-from"
              type="date"
              value={draft.from}
              onChange={(event) => setDraft({ ...draft, from: event.target.value })}
            />
          </div>

          <div className="admin-field">
            <label htmlFor="audit-to">To</label>
            <input
              id="audit-to"
              type="date"
              value={draft.to}
              onChange={(event) => setDraft({ ...draft, to: event.target.value })}
            />
          </div>

          <div className="admin-actions">
            <Button type="submit" variant="primary">
              Apply
            </Button>
            <Button type="button" variant="ghost" onClick={reset}>
              Reset
            </Button>
          </div>
        </form>

        {audit.loading && !audit.data ? (
          <Spinner label="Loading audit entries…" />
        ) : entries.length === 0 ? (
          <EmptyState
            title="No audit entries match"
            description="Admin mutations are recorded automatically. Try a wider date range."
          />
        ) : (
          <>
            <div className="admin-table-wrap" style={{ marginTop: "var(--space-3)" }}>
              <table className="admin-table">
                <thead>
                  <tr>
                    <th>When</th>
                    <th>Actor</th>
                    <th>Action</th>
                    <th>Entity</th>
                    <th aria-label="Details" />
                  </tr>
                </thead>
                <tbody>
                  {entries.map((entry) => {
                    const key = String(entry.id);
                    const open = expanded === key;
                    return (
                      <Fragment key={key}>
                        <tr>
                          <td className="admin-muted-cell admin-nowrap" title={entry.createdAt}>
                            {formatDateTime(entry.createdAt)}
                          </td>
                          <td>{entry.actorEmail ?? "system"}</td>
                          <td>
                            <Badge tone="accent">{entry.action}</Badge>
                          </td>
                          <td className="admin-muted-cell">
                            {entry.entityType}
                            {entry.entityId ? (
                              <span className="admin-mono"> · {entry.entityId.slice(0, 8)}</span>
                            ) : null}
                          </td>
                          <td>
                            <div className="admin-table__actions">
                              <Button
                                size="sm"
                                variant="ghost"
                                onClick={() => setExpanded(open ? null : key)}
                                aria-expanded={open}
                              >
                                {open ? "Hide" : "Details"}
                              </Button>
                            </div>
                          </td>
                        </tr>
                        {open ? (
                          <tr>
                            <td colSpan={5}>
                              <div className="admin-form-grid">
                                <div className="admin-stack-sm">
                                  <h4 className="admin-section-title">Before</h4>
                                  <Json value={entry.before} />
                                </div>
                                <div className="admin-stack-sm">
                                  <h4 className="admin-section-title">After</h4>
                                  <Json value={entry.after} />
                                </div>
                              </div>
                            </td>
                          </tr>
                        ) : null}
                      </Fragment>
                    );
                  })}
                </tbody>
              </table>
            </div>

            <div className="admin-pagination" style={{ marginTop: "var(--space-3)" }}>
              <span>
                {start}–{end} of {total}
              </span>
              <div className="admin-actions">
                <Button
                  size="sm"
                  onClick={() => setOffset(Math.max(0, offset - PAGE_SIZE))}
                  disabled={offset === 0}
                >
                  Previous
                </Button>
                <Button
                  size="sm"
                  onClick={() => setOffset(offset + PAGE_SIZE)}
                  disabled={end >= total}
                >
                  Next
                </Button>
              </div>
            </div>
          </>
        )}
      </section>
    </>
  );
}
