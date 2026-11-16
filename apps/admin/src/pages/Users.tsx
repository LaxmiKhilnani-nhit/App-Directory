import { EmptyState } from "@app-directory/ui";

export function Users() {
  return (
    <section className="admin-card">
      <EmptyState
        title="Users arrive in Phase 3"
        description="Bulk CSV import, per-user grants, direct app access, roles and reset codes."
      />
    </section>
  );
}
