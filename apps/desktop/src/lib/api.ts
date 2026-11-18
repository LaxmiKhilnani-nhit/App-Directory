import { invoke } from "@tauri-apps/api/core";
import type { Announcement, AppTile } from "@app-directory/types";

/** Typed wrappers around the Rust commands. There is no login or token in this client. */

export interface CommandError {
  code: string;
  message: string;
}

export interface CoreInfo {
  appName: string;
  version: string;
  apiBaseUrl: string;
  runningInTauri: boolean;
  identity: string;
}

export interface IdentityState {
  identity: string;
  shortName: string;
  name: string | null;
  department: string | null;
  cacheAvailable: boolean;
  cacheExpired: boolean;
  lastSyncedAt: string | null;
  serverReachable: boolean;
  knownUser: boolean;
}

export interface SnapshotUser {
  name: string;
  department: string | null;
}

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

export interface UpdateInfo {
  available: boolean;
  version: string;
  sha256: string;
  url: string;
}

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

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

export async function identityState(): Promise<IdentityState> {
  return invoke<IdentityState>("identity_state");
}

export async function syncSnapshot(manual: boolean): Promise<Snapshot> {
  return invoke<Snapshot>("sync_snapshot", { manual });
}

export async function cachedSnapshot(): Promise<Snapshot | null> {
  return invoke<Snapshot | null>("cached_snapshot");
}

export async function checkUpdate(): Promise<UpdateInfo> {
  return invoke<UpdateInfo>("check_update");
}

export async function applyUpdate(): Promise<void> {
  return invoke<void>("apply_update");
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
