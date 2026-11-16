import { useState, type FormEvent } from "react";
import { Button } from "@app-directory/ui";
import type { CoreInfo } from "../lib/core";

export interface LoginProps {
  core: CoreInfo | null;
  onContinue: () => void;
}

export function Login({ core, onContinue }: LoginProps) {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");

  function onSubmit(event: FormEvent) {
    event.preventDefault();
    // Phase 2 replaces this with the real /auth/login handshake in Rust.
    onContinue();
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
          <h1 className="desktop-login__title">{core?.appName ?? "App Directory"}</h1>
          <p className="desktop-login__hint">Sign in to see the applications assigned to you.</p>
        </div>

        <div className="desktop-field">
          <label htmlFor="email">Email</label>
          <input
            id="email"
            type="email"
            autoComplete="username"
            value={email}
            onChange={(event) => setEmail(event.target.value)}
            placeholder="you@company.com"
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

        <Button type="submit" variant="primary">
          Sign in
        </Button>

        <p className="desktop-login__hint">
          Forgot your password? Ask IT to issue a reset code.
        </p>

        <div className="desktop-login__status">
          <span className="desktop-dot" />
          {core?.runningInTauri
            ? `Core v${core.version} · ${core.apiBaseUrl}`
            : "Browser preview — core not attached"}
        </div>
      </form>
    </div>
  );
}
