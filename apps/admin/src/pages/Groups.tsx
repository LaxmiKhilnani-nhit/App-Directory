import { EmptyState } from "@app-directory/ui";

export function Groups() {
  return (
    <section className="admin-card">
      <EmptyState
        title="Groups arrive in Phase 3"
        description="Manual groups, auto-by-department groups, and the built-in All Users group."
      />
    </section>
  );
}
