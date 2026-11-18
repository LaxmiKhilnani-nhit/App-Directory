import { useEffect, type RefObject } from "react";

/** Ctrl+F focuses search, Esc clears it, Enter opens the first match. */
export function useSearchShortcuts(
  input: RefObject<HTMLInputElement | null>,
  query: string,
  clear: () => void,
  openFirst: () => void,
) {
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "f") {
        event.preventDefault();
        input.current?.focus();
        input.current?.select();
        return;
      }

      if (event.key === "Escape") {
        if (query) {
          event.preventDefault();
          clear();
        }
        input.current?.blur();
        return;
      }

      if (event.key === "Enter" && document.activeElement === input.current) {
        event.preventDefault();
        openFirst();
      }
    }

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [input, query, clear, openFirst]);
}
