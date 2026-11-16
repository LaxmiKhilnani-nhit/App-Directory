import type { Snapshot } from "./api";

/**
 * Browser-preview snapshot.
 *
 * `pnpm dev:desktop` serves the UI outside Tauri, where `invoke` is unavailable. Rendering
 * this fixture keeps the layout verifiable in a normal browser; nothing here ships in the
 * packaged executable because the directory only ever renders Rust-supplied data there.
 */
export function demoSnapshot(): Snapshot {
  const now = new Date();
  const lastSynced = new Date(now.getTime() - 12 * 60 * 1000);
  const validUntil = new Date(now.getTime() + 7 * 24 * 60 * 60 * 1000);

  return {
    revision: 42,
    serverTime: now.toISOString(),
    offlineValidUntil: validUntil.toISOString(),
    snapshotDays: 7,
    syncIntervalHours: 4,
    announcement: {
      text: "Payroll app moves to a new address on Monday. Bookmarks will keep working.",
      level: "info",
    },
    user: {
      name: "Preview User",
      department: "Finance",
      mustChangePassword: false,
    },
    apps: [
      {
        id: "demo-1",
        name: "Payroll",
        url: "https://example.com/payroll",
        description: "Salary slips, reimbursements and Form 16",
        category: "Finance",
        color: "#2F6FED",
        sortOrder: 10,
        offlineVisible: true,
        iconUrl: null,
        iconEtag: null,
        updatedAt: now.toISOString(),
      },
      {
        id: "demo-2",
        name: "Leave Portal",
        url: "https://example.com/leave",
        description: "Apply for leave and check your balance",
        category: "People",
        color: "#0F766E",
        sortOrder: 20,
        offlineVisible: true,
        iconUrl: null,
        iconEtag: null,
        updatedAt: now.toISOString(),
      },
      {
        id: "demo-3",
        name: "NHIT Mail",
        url: "https://example.com/mail",
        description: "Corporate email",
        category: "Productivity",
        color: "#9A3412",
        sortOrder: 30,
        offlineVisible: true,
        iconUrl: null,
        iconEtag: null,
        updatedAt: now.toISOString(),
      },
      {
        id: "demo-4",
        name: "Vendor Payments",
        url: "https://example.com/vendors",
        description: "Invoice tracking and payment approvals",
        category: "Finance",
        color: "#6D28D9",
        sortOrder: 40,
        offlineVisible: true,
        iconUrl: null,
        iconEtag: null,
        updatedAt: now.toISOString(),
      },
      {
        id: "demo-5",
        name: "Highway Asset Register",
        url: "https://example.com/assets",
        description: "Concession assets, condition surveys and defects",
        category: "Operations",
        color: "#0369A1",
        sortOrder: 50,
        offlineVisible: true,
        iconUrl: null,
        iconEtag: null,
        updatedAt: now.toISOString(),
      },
      {
        id: "demo-6",
        name: "Service Desk",
        url: "https://example.com/helpdesk",
        description: "Raise an IT ticket",
        category: "Support",
        color: "#B91C1C",
        sortOrder: 60,
        offlineVisible: true,
        iconUrl: null,
        iconEtag: null,
        updatedAt: now.toISOString(),
      },
    ],
    lastSyncedAt: lastSynced.toISOString(),
    source: "server",
    expired: false,
  };
}
