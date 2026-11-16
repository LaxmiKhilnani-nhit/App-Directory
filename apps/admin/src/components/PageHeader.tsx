import type { ReactNode } from "react";

interface PageHeaderProps {
  title: string;
  description?: string;
  actions?: ReactNode;
}

export function PageHeader({ title, description, actions }: PageHeaderProps) {
  return (
    <div className="admin-pagehead">
      <div>
        <h2 className="admin-pagehead__title">{title}</h2>
        {description ? <p className="admin-muted">{description}</p> : null}
      </div>
      {actions ? <div className="admin-pagehead__actions">{actions}</div> : null}
    </div>
  );
}

interface NoticeProps {
  tone?: "info" | "success" | "danger";
  children: ReactNode;
}

export function Notice({ tone = "info", children }: NoticeProps) {
  const suffix = tone === "info" ? "" : ` admin-notice--${tone}`;
  return <div className={`admin-notice${suffix}`}>{children}</div>;
}
