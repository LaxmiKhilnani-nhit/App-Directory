import { useEffect, useMemo, useRef, useState } from "react";
import { Button, EmptyState, SearchInput } from "@app-directory/ui";
import type { PhoneEntry } from "../lib/api";

const COPY_FEEDBACK_MS = 1500;

/** Fallback for browsers that do not expose the async clipboard API. */
function copyWithTextarea(value: string): boolean {
  const textarea = document.createElement("textarea");
  textarea.value = value;
  textarea.setAttribute("readonly", "");
  textarea.style.position = "fixed";
  textarea.style.opacity = "0";
  document.body.appendChild(textarea);
  textarea.select();

  let copied = false;
  try {
    copied = document.execCommand("copy");
  } catch {
    copied = false;
  }
  document.body.removeChild(textarea);
  return copied;
}

export function PhoneDirectory({ entries }: { entries: PhoneEntry[] }) {
  const [query, setQuery] = useState("");
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const timeoutRef = useRef<number | null>(null);

  useEffect(
    () => () => {
      if (timeoutRef.current !== null) window.clearTimeout(timeoutRef.current);
    },
    [],
  );

  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return entries;
    return entries.filter((entry) =>
      [entry.firstName, entry.lastName, entry.email, entry.phoneNo].some((field) =>
        field.toLowerCase().includes(needle),
      ),
    );
  }, [entries, query]);

  const copy = (entry: PhoneEntry) => {
    const value = entry.phoneNo;
    if (!value) return;

    const done = () => {
      setCopiedId(entry.id);
      if (timeoutRef.current !== null) window.clearTimeout(timeoutRef.current);
      timeoutRef.current = window.setTimeout(() => {
        setCopiedId(null);
        timeoutRef.current = null;
      }, COPY_FEEDBACK_MS);
    };

    if (navigator.clipboard?.writeText) {
      void navigator.clipboard.writeText(value).then(done, () => {
        if (copyWithTextarea(value)) done();
      });
    } else if (copyWithTextarea(value)) {
      done();
    }
  };

  return (
    <div className="desktop-phone">
      <div className="desktop-phone-search">
        <SearchInput
          value={query}
          onValueChange={setQuery}
          placeholder="Search phone directory…"
          aria-label="Search phone directory"
        />
      </div>

      {entries.length === 0 ? (
        <EmptyState title="No phone directory entries yet" />
      ) : visible.length === 0 ? (
        <EmptyState
          title="No matching entries"
          description="Try a different search term."
          action={
            <Button variant="secondary" size="sm" onClick={() => setQuery("")}>
              Clear search
            </Button>
          }
        />
      ) : (
        <table className="desktop-phone-table">
          <thead>
            <tr>
              <th scope="col">First Name</th>
              <th scope="col">Last Name</th>
              <th scope="col">Email</th>
              <th scope="col">Phone No</th>
            </tr>
          </thead>
          <tbody>
            {visible.map((entry) => {
              const phone = entry.phoneNo.trim();
              return (
                <tr key={entry.id}>
                  <td>{entry.firstName.trim() || "—"}</td>
                  <td>{entry.lastName.trim() || "—"}</td>
                  <td>{entry.email.trim() || "—"}</td>
                  <td>
                    <button
                      type="button"
                      className="desktop-phone-copy"
                      title="Copy phone number"
                      onClick={() => copy(entry)}
                    >
                      {phone || "—"}
                      {copiedId === entry.id ? (
                        <span className="desktop-phone-copied">Copied</span>
                      ) : null}
                    </button>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
    </div>
  );
}
