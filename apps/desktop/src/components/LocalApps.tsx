import { useEffect, useMemo, useRef, useState } from "react";
import { Button, EmptyState, SearchInput } from "@app-directory/ui";
import { type LocalApp, openLocalApp, toCommandError } from "../lib/api";

const ERROR_FEEDBACK_MS = 4000;

/** Applications discovered on this machine. Kept intentionally lightweight. */
export function LocalApps({ apps }: { apps: LocalApp[] }) {
  const [query, setQuery] = useState("");
  const [error, setError] = useState<string | null>(null);
  const timeoutRef = useRef<number | null>(null);

  useEffect(
    () => () => {
      if (timeoutRef.current !== null) window.clearTimeout(timeoutRef.current);
    },
    [],
  );

  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return apps;
    return apps.filter((app) => app.name.toLowerCase().includes(needle));
  }, [apps, query]);

  const open = (app: LocalApp) => {
    void openLocalApp(app.path).catch((failure) => {
      setError(`Could not open ${app.name}: ${toCommandError(failure).message}`);
      if (timeoutRef.current !== null) window.clearTimeout(timeoutRef.current);
      timeoutRef.current = window.setTimeout(() => {
        setError(null);
        timeoutRef.current = null;
      }, ERROR_FEEDBACK_MS);
    });
  };

  return (
    <div className="desktop-local">
      <div className="desktop-local-search">
        <SearchInput
          value={query}
          onValueChange={setQuery}
          placeholder="Search local apps…"
          aria-label="Search local apps"
        />
      </div>

      {error ? (
        <div className="desktop-banner desktop-banner--error" role="alert">{error}</div>
      ) : null}

      {apps.length === 0 ? (
        <EmptyState title="No local apps found" />
      ) : visible.length === 0 ? (
        <EmptyState
          title="No matches"
          description="Try a different search term."
          action={
            <Button variant="secondary" size="sm" onClick={() => setQuery("")}>
              Clear
            </Button>
          }
        />
      ) : (
        <div className="desktop-grid">
          {visible.map((app) => (
            <button
              key={app.id}
              type="button"
              className="desktop-local-tile"
              title={app.path}
              onClick={() => open(app)}
            >
              <span className="desktop-local-tile__monogram" aria-hidden="true">
                {app.name.charAt(0).toUpperCase() || "?"}
              </span>
              <span className="desktop-local-tile__name">{app.name}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
