#!/usr/bin/env node
/**
 * One-off migration: exports the old PostgreSQL-backed API's data into the CSV files the
 * new API reads from `infra/data/`. Run while the OLD stack is still running, then switch
 * to the CSV stack with `docker compose up -d --build`.
 *
 * Usage: ADMIN_TOKEN=<token> node scripts/export-to-csv.mjs
 */

import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const base = process.env.APPDIR_API_BASE ?? "http://127.0.0.1:8090";
const token = process.env.ADMIN_TOKEN;
if (!token) {
  console.error("ADMIN_TOKEN is required (log in to the old API first)");
  process.exit(1);
}

const out = join(root, "infra", "data");
mkdirSync(join(out, "icons"), { recursive: true });

async function api(path) {
  const response = await fetch(base + path, {
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!response.ok) throw new Error(`${path}: ${response.status} ${await response.text()}`);
  return response.json();
}

function cell(value) {
  const text = value == null ? "" : String(value);
  if (/[,"\r\n]/.test(text)) return `"${text.replaceAll('"', '""')}"`;
  return text;
}

function write(name, header, rows) {
  const lines = [header.join(",")];
  for (const row of rows) lines.push(row.map(cell).join(","));
  writeFileSync(join(out, name), lines.join("\n") + "\n");
  console.log(`${name}: ${rows.length} row(s)`);
}

const iso = (value) => (value ? new Date(value).toISOString() : "");
const flag = (value) => (value ? "1" : "0");

const apps = await api("/api/v1/admin/apps");
const groups = await api("/api/v1/admin/groups");
const usersPage = await api("/api/v1/admin/users?limit=2000");
const users = usersPage.users ?? usersPage;
const settings = await api("/api/v1/admin/settings");

const now = new Date().toISOString();

write("users.csv", ["id", "email", "name", "department", "role", "adminScope", "isActive", "mustChangePassword", "lastLoginAt", "createdAt", "updatedAt"],
  users.map((user) => [
    user.id, user.email, user.name, user.department ?? "", user.role,
    user.adminScope ? JSON.stringify(user.adminScope) : "",
    flag(user.isActive), flag(user.mustChangePassword), iso(user.lastLoginAt), iso(user.createdAt), iso(user.updatedAt),
  ]));

write("groups.csv", ["id", "name", "description", "autoDepartment", "isAllUsers", "createdAt", "updatedAt"],
  groups.map((group) => [
    group.id, group.name, group.description ?? "", group.autoDepartment ?? "",
    flag(group.isAllUsers), iso(group.createdAt), iso(group.updatedAt),
  ]));

const groupMembers = [];
const appGroups = [];
const userApps = [];

for (const group of groups) {
  const detail = await api(`/api/v1/admin/groups/${group.id}`);
  for (const userId of detail.userIds ?? []) groupMembers.push([group.id, userId, now]);
}

for (const app of apps) {
  for (const groupId of app.groupIds ?? []) appGroups.push([app.id, groupId]);
  for (const userId of app.userIds ?? []) userApps.push([userId, app.id, now]);
}

write("group_members.csv", ["groupId", "userId", "createdAt"], groupMembers);
write("app_groups.csv", ["appId", "groupId"], appGroups);
write("user_apps.csv", ["userId", "appId", "createdAt"], userApps);

write("apps.csv", ["id", "name", "url", "description", "category", "color", "sortOrder", "isEnabled", "offlineVisible", "iconSource", "iconMime", "iconFetchedAt", "iconEtag", "createdAt", "updatedAt", "deletedAt"],
  apps.map((app) => [
    app.id, app.name, app.url, app.description ?? "", app.category ?? "", app.color ?? "",
    app.sortOrder, flag(app.isEnabled), flag(app.offlineVisible), app.iconSource ?? "none",
    app.iconMime ?? "", iso(app.iconFetchedAt), app.iconEtag ?? "", iso(app.createdAt), iso(app.updatedAt), iso(app.deletedAt),
  ]));

write("settings.csv", ["key", "value", "updatedAt", "updatedBy"],
  settings.map((setting) => [setting.key, typeof setting.value === "string" ? setting.value : JSON.stringify(setting.value), iso(setting.updatedAt), setting.updatedBy ?? ""]));

write("audit.csv", ["id", "actorUserId", "actorEmail", "action", "entityType", "entityId", "before", "after", "createdAt"], []);
writeFileSync(join(out, "revision.csv"), "1\n");

console.log(`\nCSV files written to ${out}`);
