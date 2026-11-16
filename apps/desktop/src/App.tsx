import { useCallback, useEffect, useRef, useState } from "react";
import { Spinner } from "@app-directory/ui";
import {
  type CommandError,
  type CoreInfo,
  type SessionInfo,
  type SessionState,
  type Snapshot,
  cachedSnapshot,
  coreInfo,
  isTauri,
  logout as logoutCommand,
  sessionState,
  syncSnapshot,
  toCommandError,
} from "./lib/api";
import { demoSnapshot } from "./lib/demo";
import { relativeTime } from "./lib/time";
import { ChangePassword } from "./pages/ChangePassword";
import { Directory } from "./pages/Directory";
import { Login } from "./pages/Login";

/**
 * Boot flow (PLAN.md §6.3):
 *   no stored session                     → login
 *   stored session, server reachable      → live snapshot
 *   stored session, server unreachable    → cached snapshot if inside the grace window
 *   cached snapshot expired               → login with an "offline" notice
 */
type Phase = "booting" | "login" | "change-password" | "directory";

/** "Offline — last synced 3 hours ago." for the login screen. */
function offlineNotice(lastSyncedAt: string | null): string {
  return `Offline — last synced ${relativeTime(lastSyncedAt)}. Connect to the office network to refresh your apps.`;
}

export function App() {
  const [core, setCore] = useState<CoreInfo | null>(null);
  const [phase, setPhase] = useState<Phase>("booting");
  const [session, setSession] = useState<SessionState | null>(null);
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [refreshError, setRefreshError] = useState<string | null>(null);
  const [preview, setPreview] = useState(false);
  const booted = useRef(false);

  /** Tries the live sync first, then the cache when the network is down. */
  const loadDirectory = useCallback(async (allowCache: boolean): Promise<CommandError | null> => {
    try {
      const live = await syncSnapshot(false);
      setSnapshot(live);
      setRefreshError(null);
      setPhase("directory");
      return null;
    } catch (error) {
      const failure = toCommandError(error);

      if (allowCache) {
        const cached = await cachedSnapshot().catch(() => null);
        if (cached && !cached.expired) {
          setSnapshot(cached);
          setRefreshError(null);
          setPhase("directory");
          return null;
        }
      }

      return failure;
    }
  }, []);

  const boot = useCallback(async () => {
    const info = await coreInfo().catch(() => null);
    setCore(info);

    if (!info?.runningInTauri) {
      setPreview(true);
      // Dev-only: `?screen=login` / `?screen=change-password` render those screens against
      // the sample data so they can be reviewed without a Tauri build.
      const screen = import.meta.env.DEV
        ? new URLSearchParams(window.location.search).get("screen")
        : null;
      if (screen === "login") {
        setPhase("login");
        return;
      }
      if (screen === "change-password") {
        setPhase("change-password");
        return;
      }
      setSnapshot(demoSnapshot());
      setPhase("directory");
      return;
    }

    let state: SessionState;
    try {
      state = await sessionState();
    } catch (error) {
      setNotice(toCommandError(error).message);
      setPhase("login");
      return;
    }

    setSession(state);

    if (!state.signedIn) {
      if (state.cacheAvailable && state.cacheExpired) {
        setNotice(offlineNotice(state.lastSyncedAt));
      }
      setPhase("login");
      return;
    }

    if (state.mustChangePassword) {
      setPhase("change-password");
      return;
    }

    const failure = await loadDirectory(state.cacheAvailable && !state.cacheExpired);
    if (failure) {
      if (failure.code === "unauthenticated") {
        await logoutCommand().catch(() => undefined);
      }
      setNotice(
        failure.code === "network_error" && state.cacheAvailable && state.cacheExpired
          ? offlineNotice(state.lastSyncedAt)
          : failure.message,
      );
      setPhase("login");
    }
  }, [loadDirectory]);

  useEffect(() => {
    if (booted.current) return;
    booted.current = true;
    void boot();
  }, [boot]);

  /** Manual (budgeted) or background refresh from the directory screen. */
  const refresh = useCallback(async (manual: boolean) => {
    setRefreshing(true);
    setRefreshError(null);
    try {
      const live = await syncSnapshot(manual);
      setSnapshot(live);
    } catch (error) {
      const failure = toCommandError(error);
      if (failure.code === "unauthenticated") {
        await logoutCommand().catch(() => undefined);
        setSnapshot(null);
        setNotice(failure.message);
        setPhase("login");
      } else if (manual) {
        setRefreshError(failure.message);
      }
    } finally {
      setRefreshing(false);
    }
  }, []);

  // Background sync on the cadence the server asks for (PLAN.md §6.3).
  useEffect(() => {
    if (phase !== "directory" || !isTauri()) return;
    const hours = Math.max(1, snapshot?.syncIntervalHours ?? 4);
    const timer = window.setInterval(() => void refresh(false), hours * 60 * 60 * 1000);
    return () => window.clearInterval(timer);
  }, [phase, snapshot?.syncIntervalHours, refresh]);

  const signOut = useCallback(async () => {
    await logoutCommand().catch(() => undefined);
    setSnapshot(null);
    setSession(null);
    setRefreshError(null);
    setNotice(null);
    setPhase(preview ? "directory" : "login");
    if (preview) setSnapshot(demoSnapshot());
  }, [preview]);

  const handleSignedIn = useCallback(
    async (info: SessionInfo) => {
      setNotice(null);
      if (info.mustChangePassword) {
        setPhase("change-password");
        return;
      }
      const failure = await loadDirectory(false);
      if (failure) {
        setNotice(failure.message);
        setPhase("login");
      }
    },
    [loadDirectory],
  );

  const handlePasswordChanged = useCallback(async () => {
    setNotice(null);
    const failure = await loadDirectory(true);
    if (failure) {
      setNotice(failure.message);
      setPhase("login");
    }
  }, [loadDirectory]);

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

  if (phase === "login") {
    return <Login core={core} notice={notice} onSignedIn={(info) => void handleSignedIn(info)} />;
  }

  if (phase === "change-password") {
    return (
      <ChangePassword
        core={core}
        forced={session?.mustChangePassword ?? true}
        onChanged={() => void handlePasswordChanged()}
        onSignOut={() => void signOut()}
      />
    );
  }

  return (
    <Directory
      core={core}
      snapshot={snapshot ?? demoSnapshot()}
      preview={preview}
      refreshing={refreshing}
      refreshError={refreshError}
      onRefresh={() => void refresh(true)}
      onOpenError={(message) => setRefreshError(message)}
      onSignOut={() => void signOut()}
    />
  );
}
