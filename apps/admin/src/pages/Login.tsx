import { useEffect, useState, type FormEvent } from "react";
import { Button } from "@app-directory/ui";
import { api, ApiError } from "../api/client";

type ServerState = "checking" | "reachable" | "unreachable";

export function Login() {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [server, setServer] = useState<ServerState>("checking");
  const [version, setVersion] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    api
      .health()
      .then((health) => {
        if (cancelled) return;
        setServer("reachable");
        setVersion(health.version);
      })
      .catch(() => {
        if (!cancelled) setServer("unreachable");
      });
    return () => {
      cancelled = true;
    };
  }, []);

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setMessage(null);
    try {
      const session = await api.login(email, password);
      setMessage(`Signed in as ${session.user.name}. Session handling lands in Phase 2.`);
    } catch (error) {
      if (error instanceof ApiError) {
        setMessage(
          error.status === 404
            ? "Auth endpoint not implemented yet (Phase 2)."
            : `${error.code}: ${error.message}`,
        );
      } else {
        setMessage("Could not reach the server.");
      }
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="admin-login">
      <form className="admin-login__card" onSubmit={onSubmit}>
        <div>
          <h1 className="admin-login__title">App Directory</h1>
          <p className="admin-muted">Administration portal</p>
        </div>

        <div className="admin-field">
          <label htmlFor="email">Email</label>
          <input
            id="email"
            type="email"
            autoComplete="username"
            value={email}
            onChange={(event) => setEmail(event.target.value)}
            required
          />
        </div>

        <div className="admin-field">
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

        <Button type="submit" variant="primary" disabled={busy || server === "unreachable"}>
          {busy ? "Signing in…" : "Sign in"}
        </Button>

        {server === "unreachable" ? (
          <div className="admin-notice admin-notice--danger">
            Server not reachable. Check that the API is running.
          </div>
        ) : null}

        {server === "reachable" ? (
          <div className="admin-notice">
            Server online{version ? ` · v${version}` : ""}. Authentication is wired up in Phase 2.
          </div>
        ) : null}

        {message ? <div className="admin-notice">{message}</div> : null}
      </form>
    </div>
  );
}
