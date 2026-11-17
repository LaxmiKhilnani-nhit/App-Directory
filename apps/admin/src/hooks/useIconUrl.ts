import { useEffect, useState } from "react";
import { requestBlob } from "../api/client";

/**
 * Icons live behind the same bearer-token check as everything else, so `<img src>` cannot
 * load them. Fetch the bytes once per (app, etag) pair and hand the tile an object URL.
 */
const cache = new Map<string, string>();
const pending = new Map<string, Promise<string | null>>();

function loadIcon(appId: string, etag: string): Promise<string | null> {
  const key = `${appId}:${etag}`;
  const cached = cache.get(key);
  if (cached) return Promise.resolve(cached);

  const existing = pending.get(key);
  if (existing) return existing;

  const promise = requestBlob(`/api/v1/icons/${appId}`)
    .then((blob) => {
      const url = URL.createObjectURL(blob);
      cache.set(key, url);
      return url;
    })
    .catch(() => null)
    .finally(() => pending.delete(key));

  pending.set(key, promise);
  return promise;
}

export function useIconUrl(
  appId: string | undefined,
  etag: string | null | undefined,
): string | null {
  const [url, setUrl] = useState<string | null>(null);

  useEffect(() => {
    if (!appId || !etag) {
      setUrl(null);
      return;
    }

    let cancelled = false;
    loadIcon(appId, etag).then((next) => {
      if (!cancelled) setUrl(next);
    });

    return () => {
      cancelled = true;
    };
  }, [appId, etag]);

  return url;
}

/** Drops a cached icon after an upload so the next render fetches the new bytes. */
export function forgetIcon(appId: string): void {
  for (const key of [...cache.keys()]) {
    if (key.startsWith(`${appId}:`)) cache.delete(key);
  }
}
