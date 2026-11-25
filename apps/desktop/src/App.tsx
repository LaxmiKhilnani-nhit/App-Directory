import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Button, Spinner } from "@app-directory/ui";
import {
  type CommandError,
  type CoreInfo,
  type IdentityState,
  type IdentityStatus,
  type LocalApp,
  type Snapshot,
  type UpdateProgress,
  type UpdateStatus,
  UPDATE_PROGRESS_EVENT,
  EMPTY_UPDATE_STATUS,
  cachedSnapshot,
  checkForUpdate,
  clearIdentity,
  coreInfo,
  identityState,
  identityStatus,
  isTauri,
  listLocalApps,
  prepareUpdate,
  refreshLocalApps,
  syncSnapshot,
  toCommandError,
} from "./lib/api";
import { demoSnapshot } from "./lib/demo";
import { relativeTime } from "./lib/time";
import { Directory } from "./pages/Directory";
import { IdentityPrompt } from "./components/IdentityPrompt";

/**
 * Boot flow: confirm the work email once, cache it on the device, then every launch goes
 * straight to the directory. Live sync first, cached snapshot when the server is down.
 */
type Phase = "booting" | "identity" | "directory" | "error";

export function App() {
  const [core, setCore] = useState<CoreInfo | null>(null);
  const [phase, setPhase] = useState<Phase>("booting");
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [identity, setIdentity] = useState<IdentityState | null>(null);
  const [identityInfo, setIdentityInfo] = useState<IdentityStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [refreshError, setRefreshError] = useState<string | null>(null);
  const [update, setUpdate] = useState<UpdateStatus>(EMPTY_UPDATE_STATUS);
  const [progress, setProgress] = useState<UpdateProgress | null>(null);
  const [view, setView] = useState<"apps" | "local">("apps");
  const [localApps, setLocalApps] = useState<LocalApp[]>([]);
  const booted = useRef(false);

  const loadDirectory = useCallback(
    async (allowCache: boolean): Promise<CommandError | null> => {
      try {
        const live = await syncSnapshot(false);
        setSnapshot(live);
        setRefreshError(null);
        setPhase("directory");
        return null;
      } catch (failure) {
        const reason = toCommandError(failure);
        if (allowCache) {
          const cached = await cachedSnapshot().catch(() => null);
          if (cached && !cached.expired) {
            setSnapshot(cached);
            setRefreshError(null);
            setPhase("directory");
            return null;
          }
        }
        return reason;
      }
    },
    [],
  );

  const boot = useCallback(async () => {
    const info = await coreInfo().catch(() => null);
    setCore(info);

    if (!info?.runningInTauri) {
      setSnapshot(demoSnapshot());
      setPhase("directory");
      return;
    }

    let status: IdentityStatus;
    try {
      status = await identityStatus();
    } catch (failure) {
      setError(toCommandError(failure).message);
      setPhase("error");
      return;
    }
    // Silent self-update: a newer exe is downloaded now and applied on the next launch.
    void prepareUpdate()
      .then((status) => {
        setUpdate(status);
        setProgress(null);
      })
      .catch(() => {
        setUpdate({ ...EMPTY_UPDATE_STATUS, error: "Update check failed" });
        setProgress(null);
      });
    setIdentityInfo(status);
    if (!status.configured) {
      setPhase("identity");
      return;
    }

    let state: IdentityState;
    try {
      state = await identityState();
    } catch (failure) {
      setError(toCommandError(failure).message);
      setPhase("error");
      return;
    }
    setIdentity(state);

    // Local apps never block rendering: show the cached list, then rescan quietly in the background.
    void listLocalApps()
      .then(setLocalApps)
      .catch(() => undefined);
    void refreshLocalApps()
      .then(setLocalApps)
      .catch(() => undefined);

    // Show the encrypted snapshot immediately, before any network call. The live sync below
    // replaces it when it returns; if the server is down the user still has their apps.
    const cached = await cachedSnapshot().catch(() => null);
    const showedCache = Boolean(cached && !cached.expired);
    if (showedCache) {
      setSnapshot(cached);
      setPhase("directory");
    }

    const failure = await loadDirectory(state.cacheAvailable && !state.cacheExpired);
    if (failure && showedCache) {
      // Offline with a valid snapshot is a normal state; the banner in the directory explains it.
      return;
    }
    if (failure) {
      if (failure.code === "unknown_user") {
        setError(
          `${state.shortName} is not in the App Directory. Contact IT to add your account.`,
        );
      } else if (state.cacheAvailable && state.cacheExpired && failure.code === "network_error") {
        setError(
          `Offline, and the saved list expired ${relativeTime(state.lastSyncedAt)}. Connect to the office network and try again.`,
        );
      } else {
        setError(failure.message);
      }
      setPhase("error");
    }
  }, [loadDirectory]);

  useEffect(() => {
    if (!isTauri()) return;
    let unlisten: (() => void) | undefined;
    void listen<UpdateProgress>(UPDATE_PROGRESS_EVENT, (event) => setProgress(event.payload))
      .then((fn) => { unlisten = fn; })
      .catch(() => undefined);
    return () => { unlisten?.(); };
  }, []);

  useEffect(() => {
    if (booted.current) return;
    booted.current = true;
    void boot();
  }, [boot]);

  const refresh = useCallback(async (manual: boolean) => {
    setRefreshing(true);
    setRefreshError(null);
    try {
      const live = await syncSnapshot(manual);
      setSnapshot(live);
    } catch (failure) {
      const reason = toCommandError(failure);
      if (reason.code === "unknown_user") {
        setError(reason.message);
        setPhase("error");
      } else if (manual) {
        setRefreshError(reason.message);
      }
    } finally {
      setRefreshing(false);
      checkForUpdate()
        .then((status) => {
          setUpdate(status);
          setProgress(null);
        })
        .catch(() => setProgress(null));
      refreshLocalApps()
        .then(setLocalApps)
        .catch(() => undefined);
    }
  }, []);

  useEffect(() => {
    if (phase !== "directory" || !isTauri()) return;
    const hours = Math.max(1, snapshot?.syncIntervalHours ?? 4);
    const timer = window.setInterval(() => void refresh(false), hours * 60 * 60 * 1000);
    return () => window.clearInterval(timer);
  }, [phase, snapshot?.syncIntervalHours, refresh]);

  const retry = useCallback(async () => {
    setError(null);
    setPhase("booting");
    await boot();
  }, [boot]);

  const changeEmail = useCallback(async () => {
    await clearIdentity().catch(() => undefined);
    const status = await identityStatus().catch(() => null);
    if (status) setIdentityInfo(status);
    setPhase("identity");
  }, []);

  if (phase === "booting") {
    return (
      <div className="desktop-boot">
        <div className="desktop-brand__mark" aria-hidden="true">
          <span />
          <span />
          <span />
          <span />
        </div>
        <Spinner />
        <p className="desktop-boot__text">Starting App Directory…</p>
      </div>
    );
  }

  if (phase === "identity" && identityInfo) {
    return (
      <IdentityPrompt
        status={identityInfo}
        onSaved={() => {
          void boot();
        }}
      />
    );
  }

  if (phase === "error") {
    return (
      <div className="desktop-login">
        <div className="desktop-login__card">
          <div className="desktop-login__mark" aria-hidden="true">
            <span />
            <span />
            <span />
            <span />
          </div>
          <h1 className="desktop-login__title">App Directory</h1>
          <p className="desktop-boot__text">{error}</p>
          <Button variant="primary" onClick={() => void retry()}>
            Try again
          </Button>
          <div className="desktop-login__status">
            <span className="desktop-dot desktop-dot--offline" />
            {identity?.identity ?? core?.identity ?? "Windows account"}
          </div>
        </div>
      </div>
    );
  }

  return (
    <Directory
      core={core}
      snapshot={snapshot ?? demoSnapshot()}
      identity={identity}
      view={view}
      onViewChange={setView}
      localApps={localApps}
      preview={!isTauri()}
      refreshing={refreshing}
      refreshError={refreshError}
      onRefresh={() => void refresh(true)}
      onOpenError={(message) => setRefreshError(message)}
      onChangeEmail={() => void changeEmail()}
      update={update}
      progress={progress}
    />
  );
}
