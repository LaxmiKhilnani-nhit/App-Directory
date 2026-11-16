import { invoke } from "@tauri-apps/api/core";
import type { Announcement, AppTile } from "@app-directory/types";

/**
 * Typed wrappers around the Rust commands in `src-tauri/src/lib.rs`.
 *
 * Every command rejects with `{ code, message }` (see `CommandError` in `api.rs`), so the UI
 * only ever has to handle one error shape.
 */

export interface CommandError {
  code: string;
  message: string;
}

export interface CoreInfo {
  appName: string;
  version: string;
  apiBaseUrl: string;
  runningInTauri: boolean;
}

export interface SessionState {
  signedIn: boolean;
  name: string | null;
  department: string | null;
  mustChangePassword: boolean;
  cacheAvailable: boolean;
  cacheExpired: boolean;
  lastSyncedAt: string | null;
  serverReachable: boolean;
}

export interface SnapshotUser {
  name: string;
  department: string | null;
  mustChangePassword: boolean;
}

/** Mirrors `SnapshotView` in `src-tauri/src/sync.rs`. */
export interface Snapshot {
  revision: number;
  serverTime: string;
  offlineValidUntil: string;
  snapshotDays: number;
  syncIntervalHours: number;
  announcement: Announcement | null;
  user: SnapshotUser;
  apps: AppTile[];
  lastSyncedAt: string;
  source: "server" | "cache";
  expired: boolean;
}

export interface SessionInfo {
  name: string;
  department: string | null;
  mustChangePassword: boolean;
}

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** Normalises whatever `invoke` rejected with into the command error shape. */
export function toCommandError(error: unknown): CommandError {
  if (typeof error === "object" && error !== null && "message" in error) {
    const candidate = error as Partial<CommandError>;
    return {
      code: typeof candidate.code === "string" ? candidate.code : "unknown",
      message: String(candidate.message),
    };
  }
  if (typeof error === "string") {
    return { code: "unknown", message: error };
  }
  return { code: "unknown", message: "Something went wrong." };
}

export async function coreInfo(): Promise<CoreInfo> {
  return invoke<CoreInfo>("core_info");
}

export async function sessionState(): Promise<SessionState> {
  return invoke<SessionState>("session_state");
}

export async function login(email: string, password: string): Promise<SessionInfo> {
  return invoke<SessionInfo>("login", { email, password });
}

export async function logout(): Promise<void> {
  return invoke<void>("logout");
}

export async function changePassword(currentPassword: string, newPassword: string): Promise<void> {
  return invoke<void>("change_password", { currentPassword, newPassword });
}

export async function requestPasswordReset(email: string): Promise<void> {
  return invoke<void>("request_password_reset", { email });
}

export async function resetPassword(
  email: string,
  code: string,
  newPassword: string,
): Promise<void> {
  return invoke<void>("reset_password", { email, code, newPassword });
}

export async function syncSnapshot(manual: boolean): Promise<Snapshot> {
  return invoke<Snapshot>("sync_snapshot", { manual });
}

export async function cachedSnapshot(): Promise<Snapshot | null> {
  return invoke<Snapshot | null>("cached_snapshot");
}

export async function openApp(url: string): Promise<void> {
  return invoke<void>("open_app", { url });
}

/** Opens a tile in the default browser, with a `window.open` fallback for browser previews. */
export async function openTile(url: string): Promise<void> {
  if (!isTauri()) {
    window.open(url, "_blank", "noopener,noreferrer");
    return;
  }
  await openApp(url);
}
