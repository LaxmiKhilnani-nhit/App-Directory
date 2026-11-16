import { EmptyState } from "@app-directory/ui";

export function Audit() {
  return (
    <section className="admin-card">
      <EmptyState
        title="Audit log arrives in Phase 3"
        description="Every admin change is recorded with actor, entity and before/after values."
      />
    </section>
  );
}
