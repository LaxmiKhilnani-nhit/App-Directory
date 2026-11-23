import { useCallback, useMemo, useRef, useState } from "react";
import type { AppTile } from "@app-directory/types";
import {
  type CoreInfo,
  type IdentityState,
  type Snapshot,
  openTile,
  toCommandError,
} from "../lib/api";
import { useSearchShortcuts } from "../hooks/useSearchShortcuts";
import {
  AnnouncementBanner,
  AppGrid,
  AppsHeader,
  Footer,
  Notices,
  Section,
  TopBar,
} from "../components/DirectoryParts";

export interface DirectoryProps {
  core: CoreInfo | null;
  snapshot: Snapshot;
  identity: IdentityState | null;
  /** Browser preview: sample data, no core attached. */
  preview: boolean;
  refreshing: boolean;
  refreshError: string | null;
  onRefresh: () => void;
  onOpenError: (message: string) => void;
  onChangeEmail: () => void;
}

export function Directory({
  core,
  snapshot,
  identity,
  preview,
  refreshing,
  refreshError,
  onRefresh,
  onOpenError,
  onChangeEmail,
}: DirectoryProps) {
  const [query, setQuery] = useState("");
  const searchRef = useRef<HTMLInputElement>(null);

  const apps = useMemo(
    () =>
      [...snapshot.apps].sort(
        (left, right) => left.sortOrder - right.sortOrder || left.name.localeCompare(right.name),
      ),
    [snapshot.apps],
  );

  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return apps;
    return apps.filter(
      (app) =>
        app.name.toLowerCase().includes(needle) ||
        (app.description ?? "").toLowerCase().includes(needle),
    );
  }, [apps, query]);

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

  const clearFilters = useCallback(() => {
    setQuery("");
  }, []);

  useSearchShortcuts(searchRef, query, clearFilters, () => {
    if (visible.length > 0) void open(visible[0]!);
  });

  const offline = snapshot.source === "cache";

  return (
    <div className="desktop-app">
      <TopBar
        core={core}
        title="App Directory"
        offline={offline}
        refreshing={refreshing}
        name={snapshot.user.name || identity?.shortName || "Windows account"}
        meta={snapshot.user.department ?? identity?.identity ?? null}
      />

      <Section>
        <Notices
          preview={preview}
          offline={offline}
          refreshError={refreshError}
          lastSyncedAt={snapshot.lastSyncedAt}
        />
        <AnnouncementBanner value={snapshot.announcement} />
        <AppsHeader search={query} onQueryChange={setQuery} searchRef={searchRef} />
        <AppGrid apps={visible} total={apps.length} onOpen={(app) => void open(app)} onClear={clearFilters} />
      </Section>

      <Footer
        refreshing={refreshing}
        lastSyncedAt={snapshot.lastSyncedAt}
        shown={visible.length}
        total={apps.length}
        onRefresh={onRefresh}
        onChangeEmail={onChangeEmail}
        version={core?.version ?? ""}
      />
    </div>
  );
}
