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
  CategoryChips,
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

  const clearFilters = useCallback(() => {
    setQuery("");
    setCategory(null);
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
        search={query}
        onQueryChange={setQuery}
        searchRef={searchRef}
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
        <CategoryChips categories={categories} active={category} onChange={setCategory} />
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
