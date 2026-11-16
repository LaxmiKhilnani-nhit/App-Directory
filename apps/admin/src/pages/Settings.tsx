import { EmptyState } from "@app-directory/ui";

export function Settings() {
  return (
    <section className="admin-card">
      <EmptyState
        title="Settings arrive in Phase 3"
        description="Sync interval, offline grace period, announcements, and password policy."
      />
    </section>
  );
}
