import { useState, type FormEvent } from "react";
import { Button } from "@app-directory/ui";
import { type CommandError, type IdentityStatus, setIdentity, toCommandError } from "../lib/api";

export interface IdentityPromptProps {
  status: IdentityStatus;
  onSaved: () => void;
}

/** First run: confirm the work email once, then it is cached on this device. */
export function IdentityPrompt({ status, onSaved }: IdentityPromptProps) {
  const [email, setEmail] = useState(status.suggestion ?? "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await setIdentity(email);
      onSaved();
    } catch (failure) {
      setError(toCommandError(failure as CommandError).message);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="desktop-login">
      <form className="desktop-login__card" onSubmit={submit}>
        <div className="desktop-login__mark" aria-hidden="true">
          <span />
          <span />
          <span />
          <span />
        </div>

        <div>
          <h1 className="desktop-login__title">One last thing</h1>
          <p className="desktop-login__hint">
            Enter your work email once. We'll remember it on this device — your Windows account
            name doesn't always match it.
          </p>
        </div>

        {error ? (
          <div className="desktop-banner desktop-banner--error" role="alert">
            {error}
          </div>
        ) : null}

        <div className="desktop-field">
          <label htmlFor="work-email">Work email</label>
          <input
            id="work-email"
            type="email"
            autoComplete="email"
            value={email}
            onChange={(event) => setEmail(event.target.value)}
            placeholder="you@nhit.co.in"
            autoFocus
            required
          />
        </div>

        <Button type="submit" variant="primary" disabled={busy}>
          {busy ? "Checking…" : "Continue"}
        </Button>

        <p className="desktop-login__hint">
          You can change it later from the directory footer.
        </p>
      </form>
    </div>
  );
}
