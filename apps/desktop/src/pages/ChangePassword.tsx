import { useState, type FormEvent } from "react";
import { Button } from "@app-directory/ui";
import { type CoreInfo, changePassword, toCommandError } from "../lib/api";

export interface ChangePasswordProps {
  core: CoreInfo | null;
  /** True when the server flagged `must_change_password`; false for a voluntary change. */
  forced: boolean;
  onChanged: () => void;
  onSignOut: () => void;
}

const MIN_PASSWORD = 12;

export function ChangePassword({ core, forced, onChanged, onSignOut }: ChangePasswordProps) {
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    setError(null);

    if (next.length < MIN_PASSWORD) {
      setError(`Use at least ${MIN_PASSWORD} characters for the new password.`);
      return;
    }
    if (next !== confirm) {
      setError("The two new passwords do not match.");
      return;
    }
    if (next === current) {
      setError("Choose a password different from your current one.");
      return;
    }

    setBusy(true);
    try {
      await changePassword(current, next);
      onChanged();
    } catch (failure) {
      setError(toCommandError(failure).message);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="desktop-login">
      <form className="desktop-login__card" onSubmit={onSubmit}>
        <div className="desktop-login__mark" aria-hidden="true">
          <span />
          <span />
          <span />
          <span />
        </div>

        <div>
          <h1 className="desktop-login__title">
            {forced ? "Choose a new password" : "Change your password"}
          </h1>
          <p className="desktop-login__hint">
            {forced
              ? "Your account still uses a temporary password. Set your own before continuing."
              : "Pick something you have not used before on other services."}
          </p>
        </div>

        {error ? (
          <div className="desktop-banner desktop-banner--error" role="alert">
            {error}
          </div>
        ) : null}

        <div className="desktop-field">
          <label htmlFor="current-password">Current password</label>
          <input
            id="current-password"
            type="password"
            autoComplete="current-password"
            value={current}
            onChange={(event) => setCurrent(event.target.value)}
            autoFocus
            required
          />
        </div>

        <div className="desktop-field">
          <label htmlFor="next-password">New password</label>
          <input
            id="next-password"
            type="password"
            autoComplete="new-password"
            value={next}
            onChange={(event) => setNext(event.target.value)}
            placeholder={`At least ${MIN_PASSWORD} characters`}
            required
          />
        </div>

        <div className="desktop-field">
          <label htmlFor="confirm-password">Confirm new password</label>
          <input
            id="confirm-password"
            type="password"
            autoComplete="new-password"
            value={confirm}
            onChange={(event) => setConfirm(event.target.value)}
            required
          />
        </div>

        <Button type="submit" variant="primary" disabled={busy}>
          {busy ? "Saving…" : "Set password and continue"}
        </Button>

        <button type="button" className="desktop-link" onClick={onSignOut}>
          Sign out instead
        </button>

        <div className="desktop-login__status">
          <span className={`desktop-dot${core?.runningInTauri ? "" : " desktop-dot--offline"}`} />
          {core?.runningInTauri ? `v${core.version} · ${core.apiBaseUrl}` : "Browser preview"}
        </div>
      </form>
    </div>
  );
}
