import { useEffect, useState } from "react";
import { Badge, Button, Spinner } from "@app-directory/ui";
import type { Announcement, Setting } from "@app-directory/types";
import { adminApi } from "../api/admin";
import { errorMessage } from "../api/errors";
import { CheckboxField, SelectField, TextAreaField, TextField } from "../components/Form";
import { showToast } from "../components/Toast";
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
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(
    date.getDate(),
  )}T${pad(date.getHours())}:${pad(date.getMinutes())}`;
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

const NUMBER_FIELDS: Array<{ key: string; label: string; min: number; max: number; hint?: string }> = [
  { key: "sync_interval_hours", label: "Sync interval (hours)", min: 1, max: 168 },
  {
    key: "offline_grace_days",
    label: "Offline grace period (days)",
    min: 0,
    max: 90,
    hint: "0 disables offline use.",
  },
];

export function Settings() {
  const settings = useAsync(() => adminApi.settings(), []);

  const [appName, setAppName] = useState("App Directory");
  const [numbers, setNumbers] = useState<Record<string, string>>({});
  const [mailDomain, setMailDomain] = useState("nhit.co.in");
  const [defaultIdentity, setDefaultIdentity] = useState("");
  const [announcementEnabled, setAnnouncementEnabled] = useState(false);
  const [announcementText, setAnnouncementText] = useState("");
  const [announcementLevel, setAnnouncementLevel] = useState<Announcement["level"]>("info");
  const [announcementFrom, setAnnouncementFrom] = useState("");
  const [announcementTo, setAnnouncementTo] = useState("");
  const [updateEnabled, setUpdateEnabled] = useState(false);
  const [updateVersion, setUpdateVersion] = useState("");
  const [updateUrl, setUpdateUrl] = useState("");
  const [loaded, setLoaded] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

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
    setMailDomain(asText(settingValue(settings.data, "mail_domain"), "nhit.co.in"));
    setDefaultIdentity(asText(settingValue(settings.data, "default_identity"), ""));

    const raw = settingValue(settings.data, "announcement");
    const banner = raw && typeof raw === "object" ? (raw as Announcement) : null;
    setAnnouncementEnabled(Boolean(banner));
    setAnnouncementText(banner?.text ?? "");
    setAnnouncementLevel(banner?.level ?? "info");
    setAnnouncementFrom(toLocalInput(banner?.activeFrom));
    setAnnouncementTo(toLocalInput(banner?.activeTo));

    setUpdateEnabled(settingValue(settings.data, "update_enabled") === 1);
    setUpdateVersion(asText(settingValue(settings.data, "update_version"), ""));
    setUpdateUrl(asText(settingValue(settings.data, "update_url"), ""));
    setLoaded(true);
  }, [settings.data, loaded]);

  function setNumber(key: string, value: string) {
    setNumbers((previous) => ({ ...previous, [key]: value }));
  }

  async function save() {
    setError(null);

    const values: Record<string, unknown> = {
      app_name: appName.trim(),
      mail_domain: mailDomain.trim(),
      default_identity: defaultIdentity.trim(),
    };

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

    Object.assign(values, {
      update_enabled: updateEnabled ? 1 : 0,
      update_version: updateVersion.trim(),
      update_url: updateUrl.trim(),
    });

    setBusy(true);
    try {
      const updated = await adminApi.updateSettings(values);
      settings.setData(updated);
      showToast("Saved");
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
        description="Client sync, identity mapping, the announcement banner and client updates."
        actions={
          <Button variant="primary" onClick={save} disabled={busy || !loaded}>
            {busy ? "Saving…" : "Save settings"}
          </Button>
        }
      />

      {error ? <Notice tone="danger">{error}</Notice> : null}
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
            <h2 className="admin-card__title">Identity</h2>
            <p className="admin-inline-hint" style={{ marginTop: "var(--space-2)" }}>
              The client sends the current Windows username (for example <code>ad\user</code>).
              The server turns the part after the backslash into an email using this domain.
            </p>
            <div className="admin-form-grid" style={{ marginTop: "var(--space-3)" }}>
              <TextField
                label="Email domain"
                value={mailDomain}
                onChange={(event) => setMailDomain(event.target.value)}
                maxLength={253}
              />
              <TextField
                label="Fallback identity (optional)"
                value={defaultIdentity}
                onChange={(event) => setDefaultIdentity(event.target.value)}
                maxLength={253}
                hint="Used when a caller does not send a Windows identity."
              />
            </div>
          </section>

          <section className="admin-card">
            <h2 className="admin-card__title">Sync & offline</h2>
            <div className="admin-form-grid" style={{ marginTop: "var(--space-3)" }}>
              {NUMBER_FIELDS.map((field) => (
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
              <h2 className="admin-card__title">Client auto-update</h2>
              <Badge tone={updateEnabled ? "success" : "neutral"}>
                {updateEnabled ? "Enabled" : "Off"}
              </Badge>
            </div>
            <p className="admin-inline-hint" style={{ marginTop: "var(--space-2)" }}>
              With the flag on, clients automatically download the published exe on launch and
              apply it on their next start. Place the new file in the server's{" "}
              <code>release/</code> folder — the checksum is calculated automatically.
            </p>
            <div className="admin-form-stack" style={{ marginTop: "var(--space-3)" }}>
              <CheckboxField
                label="Offer the published update to clients"
                checked={updateEnabled}
                onChange={setUpdateEnabled}
              />
              <div className="admin-form-grid">
                <TextField
                  label="Published version"
                  value={updateVersion}
                  onChange={(event) => setUpdateVersion(event.target.value)}
                  placeholder="0.2.0"
                  maxLength={40}
                />
                <TextField
                  label="Download URL (optional)"
                  value={updateUrl}
                  onChange={(event) => setUpdateUrl(event.target.value)}
                  placeholder="/download/app-directory.exe"
                  maxLength={300}
                />
              </div>
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
