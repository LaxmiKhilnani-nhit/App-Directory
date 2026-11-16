import { useState, type FormEvent } from "react";
import { Button } from "@app-directory/ui";
import {
  type CoreInfo,
  type SessionInfo,
  login as loginCommand,
  requestPasswordReset,
  resetPassword,
  toCommandError,
} from "../lib/api";

type Mode = "signin" | "reset";

export interface LoginProps {
  core: CoreInfo | null;
  /** Shown when the cached snapshot has expired, or the server was unreachable. */
  notice: string | null;
  onSignedIn: (info: SessionInfo) => void;
}

const MIN_PASSWORD = 12;

export function Login({ core, notice, onSignedIn }: LoginProps) {
  const [mode, setMode] = useState<Mode>("signin");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [code, setCode] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);

  function switchMode(next: Mode) {
    setMode(next);
    setError(null);
    setInfo(null);
    setPassword("");
    setNewPassword("");
    setConfirm("");
  }

  async function onSignIn(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setError(null);
    setInfo(null);
    try {
      const session = await loginCommand(email.trim(), password);
      onSignedIn(session);
    } catch (failure) {
      setError(toCommandError(failure).message);
      setPassword("");
    } finally {
      setBusy(false);
    }
  }

  async function onRequestCode() {
    setBusy(true);
    setError(null);
    setInfo(null);
    try {
      await requestPasswordReset(email.trim());
      setMode("reset");
      setInfo(
        "Request sent. Ask IT for the reset code — it is valid for 72 hours. Then set a new password below.",
      );
    } catch (failure) {
      setError(toCommandError(failure).message);
    } finally {
      setBusy(false);
    }
  }

  async function onReset(event: FormEvent) {
    event.preventDefault();
    setError(null);
    setInfo(null);

    if (newPassword.length < MIN_PASSWORD) {
      setError(`Use at least ${MIN_PASSWORD} characters for the new password.`);
      return;
    }
    if (newPassword !== confirm) {
      setError("The two new passwords do not match.");
      return;
    }

    setBusy(true);
    try {
      await resetPassword(email.trim(), code.trim(), newPassword);
      switchMode("signin");
      setPassword("");
      setCode("");
      setInfo("Password updated. Sign in with your new password.");
    } catch (failure) {
      setError(toCommandError(failure).message);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="desktop-login">
      <div className="desktop-login__card">
        <div className="desktop-login__mark" aria-hidden="true">
          <span />
          <span />
          <span />
          <span />
        </div>

        <div>
          <h1 className="desktop-login__title">{core?.appName ?? "App Directory"}</h1>
          <p className="desktop-login__hint">
            {mode === "signin"
              ? "Sign in to see the applications assigned to you."
              : "Reset your password with a code from IT."}
          </p>
        </div>

        {notice ? <div className="desktop-banner desktop-banner--warning">{notice}</div> : null}
        {info ? <div className="desktop-banner desktop-banner--info">{info}</div> : null}
        {error ? (
          <div className="desktop-banner desktop-banner--error" role="alert">
            {error}
          </div>
        ) : null}

        {mode === "signin" ? (
          <form className="desktop-login__form" onSubmit={onSignIn}>
            <div className="desktop-field">
              <label htmlFor="email">Email</label>
              <input
                id="email"
                type="email"
                autoComplete="username"
                value={email}
                onChange={(event) => setEmail(event.target.value)}
                placeholder="you@company.com"
                autoFocus
                required
              />
            </div>

            <div className="desktop-field">
              <label htmlFor="password">Password</label>
              <input
                id="password"
                type="password"
                autoComplete="current-password"
                value={password}
                onChange={(event) => setPassword(event.target.value)}
                required
              />
            </div>

            <Button type="submit" variant="primary" disabled={busy}>
              {busy ? "Signing in…" : "Sign in"}
            </Button>

            <button
              type="button"
              className="desktop-link"
              onClick={() => {
                setMode("reset");
                setError(null);
                setInfo(null);
              }}
            >
              Forgot your password?
            </button>
          </form>
        ) : (
          <form
            className="desktop-login__form"
            onSubmit={(event) => {
              event.preventDefault();
              if (code.trim()) {
                void onReset(event);
              } else {
                void onRequestCode();
              }
            }}
          >
            <div className="desktop-field">
              <label htmlFor="reset-email">Email</label>
              <input
                id="reset-email"
                type="email"
                autoComplete="username"
                value={email}
                onChange={(event) => setEmail(event.target.value)}
                placeholder="you@company.com"
                required
              />
            </div>

            <div className="desktop-field">
              <label htmlFor="code">Reset code from IT</label>
              <input
                id="code"
                value={code}
                onChange={(event) => setCode(event.target.value.toUpperCase())}
                placeholder="ABCD-EFGH"
                maxLength={9}
                spellCheck={false}
                autoComplete="one-time-code"
              />
            </div>

            <div className="desktop-field">
              <label htmlFor="new-password">New password</label>
              <input
                id="new-password"
                type="password"
                autoComplete="new-password"
                value={newPassword}
                onChange={(event) => setNewPassword(event.target.value)}
                placeholder={`At least ${MIN_PASSWORD} characters`}
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
              />
            </div>

            <Button type="submit" variant="primary" disabled={busy}>
              {busy ? "Working…" : code.trim() ? "Set new password" : "Send reset request to IT"}
            </Button>

            <button type="button" className="desktop-link" onClick={() => switchMode("signin")}>
              Back to sign in
            </button>
          </form>
        )}

        <div className="desktop-login__status">
          <span className={`desktop-dot${core?.runningInTauri ? "" : " desktop-dot--offline"}`} />
          {core?.runningInTauri
            ? `v${core.version} · ${core.apiBaseUrl}`
            : "Browser preview — core not attached"}
        </div>
      </div>
    </div>
  );
}
