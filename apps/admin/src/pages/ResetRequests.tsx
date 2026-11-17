import { useState } from "react";
import { Badge, Button, EmptyState, Spinner } from "@app-directory/ui";
import type { IssuedCode, ResetRequest } from "@app-directory/types";
import { adminApi } from "../api/admin";
import { errorMessage } from "../api/errors";
import { CodeCallout } from "../components/CodeCallout";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { PageHeader, Notice } from "../components/PageHeader";
import { useAsync } from "../hooks/useAsync";
import { formatRelative } from "../utils/format";

type StatusFilter = "open" | "issued" | "dismissed" | "all";

function StatusBadge({ status }: { status: ResetRequest["status"] }) {
  if (status === "open") return <Badge tone="warning">Open</Badge>;
  if (status === "issued") return <Badge tone="success">Code issued</Badge>;
  return <Badge tone="neutral">Dismissed</Badge>;
}

export function ResetRequests() {
  const [status, setStatus] = useState<StatusFilter>("open");
  const requests = useAsync(
    () => adminApi.resetRequests(status === "all" ? null : status),
    [status],
  );

  const [busyId, setBusyId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [code, setCode] = useState<{ issued: IssuedCode; email: string } | null>(null);
  const [dismissing, setDismissing] = useState<ResetRequest | null>(null);

  async function issue(request: ResetRequest) {
    setBusyId(request.id);
    setError(null);
    setNotice(null);
    try {
      const issued = await adminApi.issueResetRequest(request.id);
      setCode({ issued, email: request.email });
      requests.reload();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusyId(null);
    }
  }

  async function dismiss() {
    if (!dismissing) return;
    setBusyId(dismissing.id);
    setError(null);
    setNotice(null);
    try {
      await adminApi.dismissResetRequest(dismissing.id);
      setNotice(`Request from ${dismissing.email} dismissed.`);
      setDismissing(null);
      requests.reload();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusyId(null);
    }
  }

  return (
    <>
      <PageHeader
        title="Password reset requests"
        description="People who cannot sign in raise a request from the desktop client. Issue a one-time code and pass it on through a trusted channel."
      />

      {error ? <Notice tone="danger">{error}</Notice> : null}
      {notice ? <Notice tone="success">{notice}</Notice> : null}
      {requests.error ? <Notice tone="danger">{requests.error}</Notice> : null}

      {code ? (
        <section className="admin-card">
          <h2 className="admin-card__title">Code issued</h2>
          <div style={{ marginTop: "var(--space-3)" }}>
            <CodeCallout
              code={code.issued.code}
              expiresAt={code.issued.expiresAt}
              email={code.email}
              purpose="reset"
            />
          </div>
          <div className="admin-actions" style={{ marginTop: "var(--space-3)" }}>
            <Button size="sm" onClick={() => setCode(null)}>
              Done
            </Button>
          </div>
        </section>
      ) : null}

      <section className="admin-card">
        <div className="admin-toolbar">
          <select
            value={status}
            onChange={(event) => setStatus(event.target.value as StatusFilter)}
            aria-label="Filter by request status"
          >
            <option value="open">Open requests</option>
            <option value="issued">Codes issued</option>
            <option value="dismissed">Dismissed</option>
            <option value="all">All requests</option>
          </select>
        </div>

        {requests.loading && !requests.data ? (
          <Spinner label="Loading requests…" />
        ) : !requests.data || requests.data.length === 0 ? (
          <EmptyState
            title="Nothing in the queue"
            description={
              status === "open"
                ? "No one is waiting for a reset code right now."
                : "No requests match this filter."
            }
          />
        ) : (
          <div className="admin-table-wrap" style={{ marginTop: "var(--space-3)" }}>
            <table className="admin-table">
              <thead>
                <tr>
                  <th>Person</th>
                  <th>Status</th>
                  <th>Raised</th>
                  <th>Resolved</th>
                  <th aria-label="Actions" />
                </tr>
              </thead>
              <tbody>
                {requests.data.map((request) => (
                  <tr key={request.id}>
                    <td>
                      <div className="admin-table__name">{request.name}</div>
                      <div className="admin-table__sub admin-mono">{request.email}</div>
                    </td>
                    <td>
                      <StatusBadge status={request.status} />
                    </td>
                    <td className="admin-muted-cell admin-nowrap" title={request.createdAt}>
                      {formatRelative(request.createdAt)}
                    </td>
                    <td
                      className="admin-muted-cell admin-nowrap"
                      title={request.resolvedAt ?? undefined}
                    >
                      {request.resolvedAt ? formatRelative(request.resolvedAt) : "—"}
                    </td>
                    <td>
                      <div className="admin-table__actions">
                        {request.status === "open" ? (
                          <>
                            <Button
                              size="sm"
                              variant="primary"
                              onClick={() => issue(request)}
                              disabled={busyId === request.id}
                            >
                              {busyId === request.id ? "Issuing…" : "Issue code"}
                            </Button>
                            <Button
                              size="sm"
                              onClick={() => setDismissing(request)}
                              disabled={busyId === request.id}
                            >
                              Dismiss
                            </Button>
                          </>
                        ) : (
                          <span className="admin-muted-cell">—</span>
                        )}
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
        open={Boolean(dismissing)}
        title={`Dismiss the request from ${dismissing?.name ?? "this person"}?`}
        description="No code is issued. The person can raise a new request later."
        confirmLabel="Dismiss request"
        busy={Boolean(dismissing && busyId === dismissing.id)}
        onCancel={() => setDismissing(null)}
        onConfirm={dismiss}
      />
    </>
  );
}
