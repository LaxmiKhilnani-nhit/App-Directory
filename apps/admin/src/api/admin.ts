import type {
  AdminApp,
  AdminGroup,
  AdminMe,
  AdminUser,
  AuditPage,
  GroupDetail,
  ImportReport,
  Setting,
  Stats,
  UserDetail,
  UserPage,
} from "@app-directory/types";
import { request, uploadFile } from "./client";

export interface AppInput {
  name: string;
  url: string;
  description?: string | null;
  category?: string | null;
  color?: string | null;
  sortOrder?: number;
  isEnabled?: boolean;
  offlineVisible?: boolean;
}

export type AppPatch = Partial<AppInput>;

export interface GroupInput {
  name: string;
  description?: string | null;
  autoDepartment?: string | null;
}

export type GroupPatch = Partial<GroupInput>;

export interface UserInput {
  email: string;
  name: string;
  department?: string | null;
  role?: string;
  adminScope?: { categories: string[] } | null;
}

export interface UserPatch {
  name?: string;
  department?: string | null;
  role?: string;
  isActive?: boolean;
  adminScope?: { categories: string[] } | null;
}

export interface UserQuery {
  query?: string;
  role?: string;
  active?: boolean;
  limit?: number;
  offset?: number;
}

export interface AuditQuery {
  actor?: string;
  entityType?: string;
  action?: string;
  from?: string;
  to?: string;
  limit?: number;
  offset?: number;
}

function queryString(params: Record<string, string | number | boolean | undefined | null>): string {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value === undefined || value === null || value === "") continue;
    search.set(key, String(value));
  }
  const text = search.toString();
  return text ? `?${text}` : "";
}

export const adminApi = {
  me: () => request<AdminMe>("/api/v1/admin/me"),
  stats: () => request<Stats>("/api/v1/admin/stats"),

  apps: () => request<AdminApp[]>("/api/v1/admin/apps"),
  app: (id: string) => request<AdminApp>(`/api/v1/admin/apps/${id}`),
  createApp: (input: AppInput) =>
    request<AdminApp>("/api/v1/admin/apps", {
      method: "POST",
      body: JSON.stringify(input),
    }),
  updateApp: (id: string, patch: AppPatch) =>
    request<AdminApp>(`/api/v1/admin/apps/${id}`, {
      method: "PATCH",
      body: JSON.stringify(patch),
    }),
  deleteApp: (id: string) => request<void>(`/api/v1/admin/apps/${id}`, { method: "DELETE" }),
  setAppGroups: (id: string, groupIds: string[]) =>
    request<AdminApp>(`/api/v1/admin/apps/${id}/groups`, {
      method: "PUT",
      body: JSON.stringify({ groupIds }),
    }),
  setAppUsers: (id: string, userIds: string[]) =>
    request<AdminApp>(`/api/v1/admin/apps/${id}/users`, {
      method: "PUT",
      body: JSON.stringify({ userIds }),
    }),
  refetchIcon: (id: string) =>
    request<AdminApp>(`/api/v1/admin/apps/${id}/refetch-icon`, { method: "POST" }),
  uploadIcon: (id: string, file: File) =>
    uploadFile<AdminApp>(
      `/api/v1/admin/apps/${id}/icon`,
      file,
      file.type || "application/octet-stream",
    ),

  groups: () => request<AdminGroup[]>("/api/v1/admin/groups"),
  group: (id: string) => request<GroupDetail>(`/api/v1/admin/groups/${id}`),
  createGroup: (input: GroupInput) =>
    request<AdminGroup>("/api/v1/admin/groups", {
      method: "POST",
      body: JSON.stringify(input),
    }),
  updateGroup: (id: string, patch: GroupPatch) =>
    request<AdminGroup>(`/api/v1/admin/groups/${id}`, {
      method: "PATCH",
      body: JSON.stringify(patch),
    }),
  deleteGroup: (id: string) => request<void>(`/api/v1/admin/groups/${id}`, { method: "DELETE" }),
  setGroupMembers: (id: string, userIds: string[]) =>
    request<GroupDetail>(`/api/v1/admin/groups/${id}/members`, {
      method: "PUT",
      body: JSON.stringify({ userIds }),
    }),

  users: (params: UserQuery = {}) =>
    request<UserPage>(`/api/v1/admin/users${queryString({ ...params })}`),
  user: (id: string) => request<UserDetail>(`/api/v1/admin/users/${id}`),
  createUser: (input: UserInput) =>
    request<AdminUser>("/api/v1/admin/users", {
      method: "POST",
      body: JSON.stringify(input),
    }),
  updateUser: (id: string, patch: UserPatch) =>
    request<AdminUser>(`/api/v1/admin/users/${id}`, {
      method: "PATCH",
      body: JSON.stringify(patch),
    }),
  deactivateUser: (id: string) => request<void>(`/api/v1/admin/users/${id}`, { method: "DELETE" }),
  setUserApps: (id: string, appIds: string[]) =>
    request<UserDetail>(`/api/v1/admin/users/${id}/apps`, {
      method: "PUT",
      body: JSON.stringify({ appIds }),
    }),
  importUsers: (csv: string) =>
    request<ImportReport>("/api/v1/admin/users/import", {
      method: "POST",
      body: JSON.stringify({ csv }),
    }),

  settings: () => request<Setting[]>("/api/v1/admin/settings"),
  updateSettings: (values: Record<string, unknown>) =>
    request<Setting[]>("/api/v1/admin/settings", {
      method: "PUT",
      body: JSON.stringify({ values }),
    }),

  audit: (params: AuditQuery = {}) =>
    request<AuditPage>(`/api/v1/admin/audit${queryString({ ...params })}`),
};
