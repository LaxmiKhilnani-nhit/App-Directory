import type { HealthResponse, LoginResponse, SessionUser } from "@app-directory/types";

const API_BASE = (import.meta.env.VITE_API_BASE as string | undefined) ?? "";

export class ApiError extends Error {
  readonly code: string;
  readonly status: number;

  constructor(status: number, code: string, message: string) {
    super(message);
    this.name = "ApiError";
    this.code = code;
    this.status = status;
  }
}

let authToken: string | null = null;

export function setAuthToken(token: string | null): void {
  authToken = token;
}

export function authHeaders(extra?: HeadersInit): Headers {
  const headers = new Headers(extra);
  if (authToken) headers.set("Authorization", `Bearer ${authToken}`);
  return headers;
}

async function parse<T>(response: Response): Promise<T> {
  if (response.status === 204) {
    return undefined as T;
  }

  const text = await response.text();
  let payload: unknown = null;
  try {
    payload = text ? JSON.parse(text) : null;
  } catch {
    payload = null;
  }

  if (!response.ok) {
    const envelope = payload as { error?: { code?: string; message?: string } } | null;
    throw new ApiError(
      response.status,
      envelope?.error?.code ?? "request_failed",
      envelope?.error?.message ?? response.statusText,
    );
  }

  return payload as T;
}

export async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers = authHeaders(init.headers);
  headers.set("Accept", "application/json");
  if (init.body && !(init.body instanceof Blob)) headers.set("Content-Type", "application/json");

  let response: Response;
  try {
    response = await fetch(`${API_BASE}${path}`, { ...init, headers });
  } catch {
    throw new ApiError(0, "network_error", "Could not reach the server.");
  }

  return parse<T>(response);
}

/** Raw GET for binary endpoints (icons), which cannot ride the `<img>` tag — tokens travel in a header. */
export async function requestBlob(path: string): Promise<Blob> {
  let response: Response;
  try {
    response = await fetch(`${API_BASE}${path}`, { headers: authHeaders() });
  } catch {
    throw new ApiError(0, "network_error", "Could not reach the server.");
  }

  if (!response.ok) {
    // Error bodies are JSON, so reuse the normal envelope parser for the message.
    await parse<unknown>(response);
  }

  return response.blob();
}

/** Raw POST of a file body, used by the icon upload endpoint. */
export async function uploadFile<T>(path: string, file: Blob, contentType: string): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`${API_BASE}${path}`, {
      method: "POST",
      headers: authHeaders({ "Content-Type": contentType, Accept: "application/json" }),
      body: file,
    });
  } catch {
    throw new ApiError(0, "network_error", "Could not reach the server.");
  }

  return parse<T>(response);
}

export const api = {
  health: () => request<HealthResponse>("/api/v1/healthz"),

  login: (email: string, password: string) =>
    request<LoginResponse>("/api/v1/auth/login", {
      method: "POST",
      body: JSON.stringify({ email, password }),
    }),

  me: () => request<SessionUser>("/api/v1/auth/me"),

  logout: () => request<void>("/api/v1/auth/logout", { method: "POST" }),
};
