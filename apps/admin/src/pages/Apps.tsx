import { EmptyState } from "@app-directory/ui";

export function Apps() {
  return (
    <section className="admin-card">
      <EmptyState
        title="Application list arrives in Phase 3"
        description="Create, edit, order and retire app tiles; attach groups and users; fetch favicons."
      />
    </section>
  );
}
