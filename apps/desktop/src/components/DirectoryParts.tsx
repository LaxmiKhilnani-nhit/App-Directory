import type { ReactNode, RefObject } from "react";
import type { Announcement, AppTile } from "@app-directory/types";
import { Button, EmptyState, SearchInput, Spinner, Tile } from "@app-directory/ui";
import type { CoreInfo, UpdateInfo } from "../lib/api";
import { relativeTime } from "../lib/time";

export function TopBar({
  core,
  title,
  search,
  onQueryChange,
  searchRef,
  offline,
  refreshing,
  name,
  meta,
}: {
  core: CoreInfo | null;
  title: string;
  search: string;
  onQueryChange: (value: string) => void;
  searchRef: RefObject<HTMLInputElement | null>;
  offline: boolean;
  refreshing: boolean;
  name: string;
  meta: string | null;
}) {
  return (
    <header className="desktop-topbar">
      <div className="desktop-brand">
        <div className="desktop-brand__mark" aria-hidden="true">
          <span /><span /><span /><span />
        </div>
        <span className="desktop-brand__name">{core?.appName ?? title}</span>
      </div>

      <div className="desktop-topbar__search">
        <SearchInput
          ref={searchRef}
          value={search}
          onValueChange={onQueryChange}
          placeholder="Search apps — Ctrl+F"
          aria-label="Search applications"
        />
      </div>

      <div className="desktop-topbar__actions">
        <span className="desktop-sync" title={offline ? "Working from the saved snapshot" : "Connected"}>
          <span className={`desktop-dot${offline ? " desktop-dot--offline" : ""}`} />
          {refreshing ? "Syncing…" : offline ? "Offline" : "Synced"}
        </span>
        <div className="desktop-user">
          <span className="desktop-user__name">{name}</span>
          {meta ? <span className="desktop-user__meta">{meta}</span> : null}
        </div>
      </div>
    </header>
  );
}

export function UpdateBanner({
  update,
  busy,
  error,
  onInstall,
}: {
  update: UpdateInfo | null;
  busy: boolean;
  error: string | null;
  onInstall: () => void;
}) {
  return (
    <>
      {update ? (
        <div className="desktop-banner desktop-banner--info desktop-update">
          <span>Version <strong>{update.version}</strong> is available.</span>
          <Button variant="primary" size="sm" onClick={onInstall} disabled={busy}>
            {busy ? "Updating…" : "Update now"}
          </Button>
        </div>
      ) : null}
      {error ? (
        <div className="desktop-banner desktop-banner--error" role="alert">{error}</div>
      ) : null}
    </>
  );
}

export function Notices({
  preview,
  offline,
  refreshError,
  lastSyncedAt,
}: {
  preview: boolean;
  offline: boolean;
  refreshError: string | null;
  lastSyncedAt: string;
}) {
  return (
    <>
      {preview ? (
        <div className="desktop-banner desktop-banner--info">
          Browser preview — sample apps, no server connection.
        </div>
      ) : null}
      {offline ? (
        <div className="desktop-banner desktop-banner--warning">
          Offline — showing the list saved {relativeTime(lastSyncedAt)}. Some apps may be out of date.
        </div>
      ) : null}
      {refreshError ? (
        <div className="desktop-banner desktop-banner--error" role="alert">{refreshError}</div>
      ) : null}
    </>
  );
}

export function AnnouncementBanner({ value }: { value: Announcement | null }) {
  if (!value) return null;
  const label =
    value.level === "critical" ? "Important" : value.level === "warning" ? "Notice" : "Announcement";
  return (
    <div className={`desktop-announcement desktop-announcement--${value.level}`}>
      <strong className="desktop-announcement__label">{label}</strong>
      <span>{value.text}</span>
    </div>
  );
}

export function CategoryChips({
  categories,
  active,
  onChange,
}: {
  categories: string[];
  active: string | null;
  onChange: (value: string | null) => void;
}) {
  if (categories.length < 2) return null;
  return (
    <div className="desktop-chips">
      <button type="button" className="desktop-chip" aria-pressed={active === null} onClick={() => onChange(null)}>
        All
      </button>
      {categories.map((name) => (
        <button
          key={name}
          type="button"
          className="desktop-chip"
          aria-pressed={active === name}
          onClick={() => onChange(name)}
        >
          {name}
        </button>
      ))}
    </div>
  );
}

export function AppGrid({
  apps,
  total,
  onOpen,
  onClear,
}: {
  apps: AppTile[];
  total: number;
  onOpen: (app: AppTile) => void;
  onClear: () => void;
}) {
  if (apps.length === 0) {
    return (
      <EmptyState
        title={total === 0 ? "No apps assigned yet" : "No matches"}
        description={
          total === 0
            ? "Your applications will appear here once IT assigns them. Contact IT if you expected to see something."
            : "Try a different search term or category."
        }
        action={total === 0 ? null : <Button variant="secondary" size="sm" onClick={onClear}>Clear filters</Button>}
      />
    );
  }

  return (
    <div className="desktop-grid">
      {apps.map((app) => (
        <Tile key={app.id} app={app} onOpen={onOpen} />
      ))}
    </div>
  );
}

export function Footer({
  refreshing,
  lastSyncedAt,
  shown,
  total,
  onRefresh,
  onChangeEmail,
}: {
  refreshing: boolean;
  lastSyncedAt: string;
  shown: number;
  total: number;
  onRefresh: () => void;
  onChangeEmail: () => void;
}) {
  const count = shown === total
    ? `${total} app${total === 1 ? "" : "s"}`
    : `${shown} of ${total} apps`;

  return (
    <footer className="desktop-footer">
      <span>
        {refreshing ? (
          <span className="desktop-footer__busy"><Spinner /> Refreshing…</span>
        ) : (
          `Last synced ${relativeTime(lastSyncedAt)}`
        )}
      </span>
      <span>{count}</span>
      <div className="desktop-footer__actions">
        <Button variant="ghost" size="sm" onClick={onChangeEmail}>
          Change email
        </Button>
        <Button variant="secondary" size="sm" onClick={onRefresh} disabled={refreshing}>
          Refresh now
        </Button>
      </div>
    </footer>
  );
}

export function Section({ children }: { children: ReactNode }) {
  return <main className="desktop-main">{children}</main>;
}
