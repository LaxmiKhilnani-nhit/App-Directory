import type { ReactNode, RefObject } from "react";
import type { Announcement, AppTile } from "@app-directory/types";
import { Button, EmptyState, SearchInput, Spinner, Tile } from "@app-directory/ui";
import type { CoreInfo, LocalApp, UpdateInfo, UpdateProgress, UpdateStatus } from "../lib/api";
import { relativeTime } from "../lib/time";

export function TopBar({
  core,
  title,
  offline,
  refreshing,
  name,
  meta,
  view,
  onViewChange,
  localAppsEnabled,
}: {
  core: CoreInfo | null;
  title: string;
  offline: boolean;
  refreshing: boolean;
  name: string;
  meta: string | null;
  view: "apps" | "local";
  onViewChange: (view: "apps" | "local") => void;
  localApps: LocalApp[];
  localAppsEnabled: boolean;
}) {
  return (
    <header className="desktop-topbar">
      <div className="desktop-brand">
        <div className="desktop-brand__mark" aria-hidden="true">
          <span /><span /><span /><span />
        </div>
        <span className="desktop-brand__name">{core?.appName ?? title}</span>
      </div>

      <div className="desktop-topbar__actions">
        {localAppsEnabled ? (
          <Button
            variant="secondary"
            size="sm"
            onClick={() => onViewChange(view === "local" ? "apps" : "local")}
          >
            {view === "local" ? "Back to Apps" : "Local Apps"}
          </Button>
        ) : null}
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

export function AppsHeader({
  search,
  onQueryChange,
  searchRef,
}: {
  search: string;
  onQueryChange: (value: string) => void;
  searchRef: RefObject<HTMLInputElement | null>;
}) {
  return (
    <div className="desktop-apps-header">
      <div className="desktop-tabs" role="tablist" aria-label="Directory sections">
        <span className="desktop-tab desktop-tab--active" role="tab" aria-selected="true">
          Apps
        </span>
      </div>
      <div className="desktop-apps-search">
        <SearchInput
          ref={searchRef}
          value={search}
          onValueChange={onQueryChange}
          placeholder="Search apps — Ctrl+F"
          aria-label="Search applications"
        />
      </div>
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
            : "Try a different search term."
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
  version,
  update,
  progress,
}: {
  refreshing: boolean;
  lastSyncedAt: string;
  shown: number;
  total: number;
  onRefresh: () => void;
  onChangeEmail: () => void;
  version: string;
  update: UpdateStatus;
  progress: UpdateProgress | null;
}) {
  const count = shown === total
    ? `${total} app${total === 1 ? "" : "s"}`
    : `${shown} of ${total} apps`;
  const updateLabel = update.staged
    ? `Update ${update.version} ready — restart to apply`
    : update.error
      ? "Update check failed"
      : update.checking
        ? "Checking for updates…"
        : update.available
          ? `Update ${update.version} available`
          : null;

  return (
    <footer className="desktop-footer">
      <span>
        {refreshing ? (
          <span className="desktop-footer__busy"><Spinner /> Refreshing…</span>
        ) : (
          `Last synced ${relativeTime(lastSyncedAt)}`
        )}
        {progress ? (
          <span className="desktop-update-progress" title="Downloading update">
            <span className="desktop-update-progress__track">
              <span
                className="desktop-update-progress__fill"
                style={{ width: `${Math.max(0, Math.min(100, progress.percent))}%` }}
              />
            </span>
            <span className="desktop-update-progress__label">Updating {progress.percent}%</span>
          </span>
        ) : null}
        {updateLabel ? (
          <span className="desktop-update-status" title={update.error ?? undefined}>
            {updateLabel}
          </span>
        ) : null}
      </span>
      <span>
        {count} · v{version}
      </span>
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
