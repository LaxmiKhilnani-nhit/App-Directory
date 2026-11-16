import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { AppTile } from "@app-directory/types";
import { Button, EmptyState, SearchInput, Spinner, Tile } from "@app-directory/ui";
import { type CoreInfo, type Snapshot, openTile, toCommandError } from "../lib/api";
import { relativeTime } from "../lib/time";

export interface DirectoryProps {
  core: CoreInfo | null;
  snapshot: Snapshot;
  /** Browser preview: sample data, no core attached. */
  preview: boolean;
  refreshing: boolean;
  refreshError: string | null;
  onRefresh: () => void;
  onOpenError: (message: string) => void;
  onSignOut: () => void;
}

export function Directory({
  core,
  snapshot,
  preview,
  refreshing,
  refreshError,
  onRefresh,
  onOpenError,
  onSignOut,
}: DirectoryProps) {
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState<string | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);

  const apps = useMemo(
    () =>
      [...snapshot.apps].sort(
        (left, right) => left.sortOrder - right.sortOrder || left.name.localeCompare(right.name),
      ),
    [snapshot.apps],
  );

  const categories = useMemo(
    () => Array.from(new Set(apps.map((app) => app.category).filter(Boolean) as string[])).sort(),
    [apps],
  );

  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return apps.filter((app) => {
      const matchesCategory = !category || app.category === category;
      const matchesQuery =
        !needle ||
        app.name.toLowerCase().includes(needle) ||
        (app.description ?? "").toLowerCase().includes(needle) ||
        (app.category ?? "").toLowerCase().includes(needle);
      return matchesCategory && matchesQuery;
    });
  }, [apps, query, category]);

  const open = useCallback(
    async (app: AppTile) => {
      try {
        await openTile(app.url);
      } catch (failure) {
        onOpenError(toCommandError(failure).message);
      }
    },
    [onOpenError],
  );

  // Ctrl+F to search, Esc to clear, Enter to open the first match (PLAN.md §6.3).
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "f") {
        event.preventDefault();
        searchRef.current?.focus();
        searchRef.current?.select();
        return;
      }

      if (event.key === "Escape") {
        if (query) {
          event.preventDefault();
          setQuery("");
        }
        searchRef.current?.blur();
        return;
      }

      if (
        event.key === "Enter" &&
        document.activeElement === searchRef.current &&
        visible.length > 0
      ) {
        event.preventDefault();
        void open(visible[0]!);
      }
    }

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [open, query, visible]);

  const offline = snapshot.source === "cache";
  const announcement = snapshot.announcement;

  return (
    <div className="desktop-app">
      <header className="desktop-topbar">
        <div className="desktop-brand">
          <div className="desktop-brand__mark" aria-hidden="true">
            <span />
            <span />
            <span />
            <span />
          </div>
          <span className="desktop-brand__name">{core?.appName ?? "App Directory"}</span>
        </div>

        <div className="desktop-topbar__search">
          <SearchInput
            ref={searchRef}
            value={query}
            onValueChange={setQuery}
            placeholder="Search apps — Ctrl+F"
            aria-label="Search applications"
          />
        </div>

        <div className="desktop-topbar__actions">
          <span
            className="desktop-sync"
            title={offline ? "Working from the saved snapshot" : "Connected"}
          >
            <span className={`desktop-dot${offline ? " desktop-dot--offline" : ""}`} />
            {refreshing ? "Syncing…" : offline ? "Offline" : "Synced"}
          </span>
          <div className="desktop-user">
            <span className="desktop-user__name">{snapshot.user.name}</span>
            {snapshot.user.department ? (
              <span className="desktop-user__meta">{snapshot.user.department}</span>
            ) : null}
          </div>
          <Button variant="ghost" size="sm" onClick={onSignOut}>
            Sign out
          </Button>
        </div>
      </header>

      <main className="desktop-main">
        {preview ? (
          <div className="desktop-banner desktop-banner--info">
            Browser preview — sample apps, no server connection.
          </div>
        ) : null}

        {offline ? (
          <div className="desktop-banner desktop-banner--warning">
            Offline — showing the list saved {relativeTime(snapshot.lastSyncedAt)}. Some apps may be
            out of date.
          </div>
        ) : null}

        {refreshError ? (
          <div className="desktop-banner desktop-banner--error" role="alert">
            {refreshError}
          </div>
        ) : null}

        {announcement ? (
          <div className={`desktop-announcement desktop-announcement--${announcement.level}`}>
            <strong className="desktop-announcement__label">
              {announcement.level === "critical"
                ? "Important"
                : announcement.level === "warning"
                  ? "Notice"
                  : "Announcement"}
            </strong>
            <span>{announcement.text}</span>
          </div>
        ) : null}

        {categories.length > 1 ? (
          <div className="desktop-chips">
            <button
              type="button"
              className="desktop-chip"
              aria-pressed={category === null}
              onClick={() => setCategory(null)}
            >
              All
            </button>
            {categories.map((name) => (
              <button
                key={name}
                type="button"
                className="desktop-chip"
                aria-pressed={category === name}
                onClick={() => setCategory(name)}
              >
                {name}
              </button>
            ))}
          </div>
        ) : null}

        {visible.length === 0 ? (
          <EmptyState
            title={apps.length === 0 ? "No apps assigned yet" : "No matches"}
            description={
              apps.length === 0
                ? "Your applications will appear here once IT assigns them. Contact IT if you expected to see something."
                : "Try a different search term or category."
            }
            action={
              apps.length === 0 ? null : (
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() => {
                    setQuery("");
                    setCategory(null);
                  }}
                >
                  Clear filters
                </Button>
              )
            }
          />
        ) : (
          <div className="desktop-grid">
            {visible.map((app) => (
              <Tile key={app.id} app={app} onOpen={(target) => void open(target)} />
            ))}
          </div>
        )}
      </main>

      <footer className="desktop-footer">
        <span>
          {refreshing ? (
            <span className="desktop-footer__busy">
              <Spinner /> Refreshing…
            </span>
          ) : (
            `Last synced ${relativeTime(snapshot.lastSyncedAt)}`
          )}
        </span>
        <span>
          {visible.length === apps.length
            ? `${apps.length} app${apps.length === 1 ? "" : "s"}`
            : `${visible.length} of ${apps.length} apps`}
        </span>
        <Button variant="secondary" size="sm" onClick={onRefresh} disabled={refreshing}>
          Refresh now
        </Button>
      </footer>
    </div>
  );
}
