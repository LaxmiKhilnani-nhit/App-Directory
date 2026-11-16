import type { ReactNode } from "react";

export interface EmptyStateProps {
  title: string;
  description?: string;
  action?: ReactNode;
}

export function EmptyState({ title, description, action }: EmptyStateProps) {
  return (
    <div className="ad-empty">
      <span className="ad-empty__title">{title}</span>
      {description ? <span>{description}</span> : null}
      {action}
    </div>
  );
}
