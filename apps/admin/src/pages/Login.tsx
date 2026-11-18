import { useEffect, useState, type FormEvent } from "react";
import { Navigate, useNavigate } from "react-router-dom";
import { Button, Spinner } from "@app-directory/ui";
import { api, ApiError } from "../api/client";
import { useAuth } from "../auth";

type ServerState = "checking" | "reachable" | "unreachable";

export function Login() {
  const { user, loading, signIn } = useAuth();
  const navigate = useNavigate();

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

  if (loading) {
    return (
      <div className="admin-loading">
        <Spinner label="Checking your session…" />
      </div>
    );
  }

  if (user) {
    return <Navigate to={user.mustChangePassword ? "/change-password" : "/dashboard"} replace />;
  }

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setMessage(null);

    try {
      const signedIn = await signIn(email.trim(), password);
      navigate(signedIn.mustChangePassword ? "/change-password" : "/dashboard", { replace: true });
    } catch (error) {
      if (error instanceof ApiError) {
        setMessage(
          error.code === "account_locked" || error.code === "account_inactive"
            ? error.message
            : error.code === "invalid_credentials"
              ? "Email or password is incorrect."
              : error.code === "network_error"
                ? "Could not reach the server. Check that the API is running."
                : `${error.code}: ${error.message}`,
        );
      } else {
        setMessage("Could not reach the server.");
      }
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
            autoFocus
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

        {server === "reachable" && !message ? (
          <div className="admin-notice">Server online{version ? ` · v${version}` : ""}</div>
        ) : null}

        {message ? <div className="admin-notice admin-notice--danger">{message}</div> : null}
      </form>
    </div>
  );
}
