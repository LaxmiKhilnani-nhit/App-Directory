import { useMemo, useState } from "react";
import type { AppTile } from "@app-directory/types";
import { Button, EmptyState, SearchInput, Tile } from "@app-directory/ui";
import type { CoreInfo } from "../lib/core";

export interface DirectoryProps {
  core: CoreInfo | null;
  onSignOut: () => void;
}

export function Directory({ core, onSignOut }: DirectoryProps) {
  // Phase 5 replaces this with the decrypted local snapshot / live sync payload.
  const apps: AppTile[] = [];
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState<string | null>(null);

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
          <SearchInput value={query} onValueChange={setQuery} />
        </div>

        <div className="desktop-topbar__actions">
          <span className="desktop-sync">
            <span className="desktop-dot" />
            Not synced yet
          </span>
          <Button variant="ghost" size="sm" onClick={onSignOut}>
            Sign out
          </Button>
        </div>
      </header>

      <main className="desktop-main">
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
            title={apps.length === 0 ? "No apps yet" : "No matches"}
            description={
              apps.length === 0
                ? "Your assigned applications will appear here once the directory is connected (Phase 5)."
                : "Try a different search term or category."
            }
          />
        ) : (
          <div className="desktop-grid">
            {visible.map((app) => (
              <Tile key={app.id} app={app} onOpen={(target) => window.open(target.url, "_blank")} />
            ))}
          </div>
        )}
      </main>

      <footer className="desktop-footer">
        <span>Never synced</span>
        <span>{visible.length} apps</span>
      </footer>
    </div>
  );
}
