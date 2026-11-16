import { invoke } from "@tauri-apps/api/core";

export interface CoreInfo {
  appName: string;
  version: string;
  apiBaseUrl: string;
  runningInTauri: boolean;
}

/** Reads static info from the Rust side; falls back to a stub when opened in a browser. */
export async function readCoreInfo(): Promise<CoreInfo> {
  try {
    return await invoke<CoreInfo>("core_info");
  } catch {
    return {
      appName: "App Directory",
      version: "0.1.0",
      apiBaseUrl: "http://127.0.0.1:8090",
      runningInTauri: false,
    };
  }
}
