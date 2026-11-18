import { useCallback, useEffect, useState } from "react";
import {
  type CommandError,
  type UpdateInfo,
  applyUpdate,
  checkUpdate,
  toCommandError,
} from "../lib/api";

/** Watches the server's update flag and installs a published client when asked. */
export function useUpdate(enabled: boolean, revision: number) {
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!enabled) return;
    let active = true;
    checkUpdate()
      .then((info) => {
        if (active) setUpdate(info.available ? info : null);
      })
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, [enabled, revision]);

  const install = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      await applyUpdate();
    } catch (failure) {
      setError(toCommandError(failure as CommandError).message);
      setBusy(false);
    }
  }, []);

  return { update, busy, error, install };
}
