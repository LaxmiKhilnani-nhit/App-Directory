import { useEffect, type ReactNode } from "react";

interface DrawerProps {
  title: string;
  subtitle?: string;
  size?: "md" | "wide";
  onClose: () => void;
  footer?: ReactNode;
  children: ReactNode;
}

export function Drawer({ title, subtitle, size = "md", onClose, footer, children }: DrawerProps) {
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") onClose();
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  return (
    <div className="admin-overlay" onMouseDown={onClose} role="presentation">
      <aside
        className={`admin-drawer${size === "wide" ? " admin-drawer--wide" : ""}`}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <header className="admin-drawer__head">
          <div>
            <h2 className="admin-drawer__title">{title}</h2>
            {subtitle ? <p className="admin-muted">{subtitle}</p> : null}
          </div>
          <button type="button" className="admin-icon-btn" onClick={onClose} aria-label="Close">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
              <path
                d="M4 4l8 8m0-8l-8 8"
                stroke="currentColor"
                strokeWidth="1.6"
                strokeLinecap="round"
              />
            </svg>
          </button>
        </header>

        <div className="admin-drawer__body">{children}</div>

        {footer ? <footer className="admin-drawer__foot">{footer}</footer> : null}
      </aside>
    </div>
  );
}
