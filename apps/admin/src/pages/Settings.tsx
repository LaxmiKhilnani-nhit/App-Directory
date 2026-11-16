import { useEffect, useState } from "react";
import { Badge, Button, Spinner } from "@app-directory/ui";
import type { Announcement, Setting } from "@app-directory/types";
import { adminApi } from "../api/admin";
import { errorMessage } from "../api/errors";
import { CheckboxField, SelectField, TextAreaField, TextField } from "../components/Form";
import { PageHeader, Notice } from "../components/PageHeader";
import { useAsync } from "../hooks/useAsync";
import { formatDateTime } from "../utils/format";

function settingValue(settings: Setting[] | null, key: string): unknown {
  return settings?.find((setting) => setting.key === key)?.value;
}

function asText(value: unknown, fallback: string): string {
  if (typeof value === "string") return value;
  if (typeof value === "number") return String(value);
  return fallback;
}

function toLocalInput(value: string | null | undefined): string {
  if (!value) return "";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  const pad = (input: number) => String(input).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(
    date.getHours(),
  )}:${pad(date.getMinutes())}`;
}

function fromLocalInput(value: string): string | undefined {
  if (!value) return undefined;
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return undefined;
  return date.toISOString();
}

function previewClass(level: Announcement["level"]): string {
  if (level === "warning") return "admin-notice admin-notice--warning";
  if (level === "critical") return "admin-notice admin-notice--danger";
  return "admin-notice";
}

const NUMBER_FIELDS: Array<{
  key: string;
  label: string;
  min: number;
  max: number;
  hint?: string;
}> = [
  { key: "sync_interval_hours", label: "Sync interval (hours)", min: 1, max: 168 },
  {
    key: "offline_grace_days",
    label: "Offline grace period (days)",
    min: 0,
    max: 90,
    hint: "0 disables offline use.",
  },
  { key: "password_min_length", label: "Minimum password length", min: 8, max: 200 },
  {
    key: "login_lockout_attempts",
    label: "Failed sign-ins before lockout",
    min: 0,
    max: 100,
    hint: "0 disables lockout.",
  },
  { key: "login_lockout_minutes", label: "Lockout duration (minutes)", min: 1, max: 1440 },
];

export function Settings() {
  const settings = useAsync(() => adminApi.settings(), []);

  const [appName, setAppName] = useState("App Directory");
  const [numbers, setNumbers] = useState<Record<string, string>>({});
  const [announcementEnabled, setAnnouncementEnabled] = useState(false);
  const [announcementText, setAnnouncementText] = useState("");
  const [announcementLevel, setAnnouncementLevel] = useState<Announcement["level"]>("info");
  const [announcementFrom, setAnnouncementFrom] = useState("");
  const [announcementTo, setAnnouncementTo] = useState("");

  const [loaded, setLoaded] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    if (!settings.data || loaded) return;

    setAppName(asText(settingValue(settings.data, "app_name"), "App Directory"));
    setNumbers(
      Object.fromEntries(
        NUMBER_FIELDS.map((field) => [
          field.key,
          asText(settingValue(settings.data, field.key), String(field.min)),
        ]),
      ),
    );

    const raw = settingValue(settings.data, "announcement");
    const banner = raw && typeof raw === "object" ? (raw as Announcement) : null;
    setAnnouncementEnabled(Boolean(banner));
    setAnnouncementText(banner?.text ?? "");
    setAnnouncementLevel(banner?.level ?? "info");
    setAnnouncementFrom(toLocalInput(banner?.activeFrom));
    setAnnouncementTo(toLocalInput(banner?.activeTo));
    setLoaded(true);
  }, [settings.data, loaded]);

  function setNumber(key: string, value: string) {
    setNumbers((previous) => ({ ...previous, [key]: value }));
  }

  async function save() {
    setError(null);
    setNotice(null);

    const values: Record<string, unknown> = { app_name: appName.trim() };

    for (const field of NUMBER_FIELDS) {
      const parsed = Number(numbers[field.key]);
      if (!Number.isInteger(parsed) || parsed < field.min || parsed > field.max) {
        setError(`${field.label} must be a whole number between ${field.min} and ${field.max}.`);
        return;
      }
      values[field.key] = parsed;
    }

    if (announcementEnabled) {
      if (!announcementText.trim() || announcementText.trim().length > 300) {
        setError("Announcement text must be between 1 and 300 characters.");
        return;
      }
      values.announcement = {
        text: announcementText.trim(),
        level: announcementLevel,
        activeFrom: fromLocalInput(announcementFrom),
        activeTo: fromLocalInput(announcementTo),
      };
    } else {
      values.announcement = null;
    }

    setBusy(true);
    try {
      const updated = await adminApi.updateSettings(values);
      settings.setData(updated);
      setNotice("Settings saved. Clients pick the change up on their next sync.");
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  }

  const lastUpdated = (settings.data ?? [])
    .map((setting) => setting.updatedAt)
    .sort()
    .at(-1);

  return (
    <>
      <PageHeader
        title="Settings"
        description="Client sync behaviour, the sign-in policy and the announcement banner."
        actions={
          <Button variant="primary" onClick={save} disabled={busy || !loaded}>
            {busy ? "Saving…" : "Save settings"}
          </Button>
        }
      />

      {error ? <Notice tone="danger">{error}</Notice> : null}
      {notice ? <Notice tone="success">{notice}</Notice> : null}
      {settings.error ? <Notice tone="danger">{settings.error}</Notice> : null}

      {settings.loading && !settings.data ? (
        <section className="admin-card">
          <Spinner label="Loading settings…" />
        </section>
      ) : null}

      {settings.data ? (
        <>
          <section className="admin-card">
            <h2 className="admin-card__title">Application</h2>
            <div className="admin-form-stack" style={{ marginTop: "var(--space-3)" }}>
              <TextField
                label="Application name"
                value={appName}
                onChange={(event) => setAppName(event.target.value)}
                maxLength={80}
                hint="Shown in the client window title and about screen."
              />
            </div>
          </section>

          <section className="admin-card">
            <h2 className="admin-card__title">Sync & offline</h2>
            <div className="admin-form-grid" style={{ marginTop: "var(--space-3)" }}>
              {NUMBER_FIELDS.slice(0, 2).map((field) => (
                <TextField
                  key={field.key}
                  label={field.label}
                  type="number"
                  min={field.min}
                  max={field.max}
                  value={numbers[field.key] ?? ""}
                  onChange={(event) => setNumber(field.key, event.target.value)}
                  hint={field.hint}
                />
              ))}
            </div>
          </section>

          <section className="admin-card">
            <h2 className="admin-card__title">Sign-in policy</h2>
            <div className="admin-form-grid" style={{ marginTop: "var(--space-3)" }}>
              {NUMBER_FIELDS.slice(2).map((field) => (
                <TextField
                  key={field.key}
                  label={field.label}
                  type="number"
                  min={field.min}
                  max={field.max}
                  value={numbers[field.key] ?? ""}
                  onChange={(event) => setNumber(field.key, event.target.value)}
                  hint={field.hint}
                />
              ))}
            </div>
          </section>

          <section className="admin-card">
            <div className="admin-pagehead">
              <h2 className="admin-card__title">Announcement banner</h2>
              <Badge tone={announcementEnabled ? "success" : "neutral"}>
                {announcementEnabled ? "Active" : "Off"}
              </Badge>
            </div>

            <div className="admin-form-stack" style={{ marginTop: "var(--space-3)" }}>
              <CheckboxField
                label="Show a banner in the client"
                hint="Useful for maintenance windows and important notices."
                checked={announcementEnabled}
                onChange={setAnnouncementEnabled}
              />

              {announcementEnabled ? (
                <>
                  <TextAreaField
                    label="Message"
                    value={announcementText}
                    onChange={(event) => setAnnouncementText(event.target.value)}
                    maxLength={300}
                    placeholder="Payroll will be unavailable on Saturday morning."
                  />

                  <div className="admin-form-grid">
                    <SelectField
                      label="Tone"
                      value={announcementLevel}
                      onChange={(event) =>
                        setAnnouncementLevel(event.target.value as Announcement["level"])
                      }
                    >
                      <option value="info">Info</option>
                      <option value="warning">Warning</option>
                      <option value="critical">Critical</option>
                    </SelectField>

                    <TextField
                      label="Starts at (optional)"
                      type="datetime-local"
                      value={announcementFrom}
                      onChange={(event) => setAnnouncementFrom(event.target.value)}
                    />

                    <TextField
                      label="Ends at (optional)"
                      type="datetime-local"
                      value={announcementTo}
                      onChange={(event) => setAnnouncementTo(event.target.value)}
                    />
                  </div>

                  <div className={previewClass(announcementLevel)}>
                    <strong>Preview:</strong>{" "}
                    {announcementText.trim() || "Your message appears here."}
                  </div>
                </>
              ) : null}
            </div>
          </section>

          <p className="admin-inline-hint">
            Values are applied on the server immediately. Last changed {formatDateTime(lastUpdated)}
            .
          </p>
        </>
      ) : null}
    </>
  );
}
