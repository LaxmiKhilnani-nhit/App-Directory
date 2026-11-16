import { Button, EmptyState } from "@app-directory/ui";

export function Dashboard() {
  return (
    <>
      <div className="admin-grid">
        {[
          ["Users", "0"],
          ["Apps", "0"],
          ["Groups", "0"],
          ["Open reset requests", "0"],
        ].map(([label, value]) => (
          <div className="admin-metric" key={label}>
            <div className="admin-metric__label">{label}</div>
            <div className="admin-metric__value">{value}</div>
          </div>
        ))}
      </div>

      <section className="admin-card">
        <h2 className="admin-card__title">Getting started</h2>
        <p className="admin-muted">
          Phase 0 scaffold. Counts, recent activity and the reset queue populate once the data
          layer (Phase 1) and admin API (Phase 3) are in place.
        </p>
        <div style={{ marginTop: "var(--space-4)" }}>
          <Button variant="primary" disabled>
            Add your first app
          </Button>
        </div>
      </section>

      <section className="admin-card">
        <EmptyState
          title="No activity yet"
          description="Admin actions will appear here as they are recorded in the audit log."
        />
      </section>
    </>
  );
}
