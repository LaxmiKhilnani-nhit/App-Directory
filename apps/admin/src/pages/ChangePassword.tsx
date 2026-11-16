import { useState, type FormEvent } from "react";
import { useNavigate } from "react-router-dom";
import { Button } from "@app-directory/ui";
import { api, ApiError } from "../api/client";
import { useAuth } from "../auth";

export function ChangePassword() {
  const { user, replaceUser, signOut } = useAuth();
  const navigate = useNavigate();

  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const forced = user?.mustChangePassword ?? false;
  const mismatch = confirmPassword.length > 0 && newPassword !== confirmPassword;

  async function onSignOut() {
    await signOut();
    navigate("/login", { replace: true });
  }

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    setMessage(null);

    if (newPassword !== confirmPassword) {
      setMessage("The new passwords do not match.");
      return;
    }

    setBusy(true);
    try {
      await api.changePassword(currentPassword, newPassword);
      if (user) replaceUser({ ...user, mustChangePassword: false });
      navigate("/dashboard", { replace: true });
    } catch (error) {
      if (error instanceof ApiError) {
        setMessage(
          error.code === "invalid_password"
            ? "Current password is incorrect."
            : error.message,
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
          <h1 className="admin-login__title">
            {forced ? "Set a new password" : "Change password"}
          </h1>
          <p className="admin-muted">
            {forced
              ? "This account was created with a temporary password. Choose your own to continue."
              : `Signed in as ${user?.email ?? ""}.`}
          </p>
        </div>

        <div className="admin-field">
          <label htmlFor="current">Current password</label>
          <input
            id="current"
            type="password"
            autoComplete="current-password"
            value={currentPassword}
            onChange={(event) => setCurrentPassword(event.target.value)}
            autoFocus
            required
          />
        </div>

        <div className="admin-field">
          <label htmlFor="new">New password</label>
          <input
            id="new"
            type="password"
            autoComplete="new-password"
            value={newPassword}
            onChange={(event) => setNewPassword(event.target.value)}
            minLength={12}
            required
          />
        </div>

        <div className="admin-field">
          <label htmlFor="confirm">Confirm new password</label>
          <input
            id="confirm"
            type="password"
            autoComplete="new-password"
            value={confirmPassword}
            onChange={(event) => setConfirmPassword(event.target.value)}
            minLength={12}
            required
          />
        </div>

        <div className="admin-actions">
          <Button
            type="submit"
            variant="primary"
            disabled={busy || mismatch || newPassword.length < 12}
          >
            {busy ? "Saving…" : "Save password"}
          </Button>
          <Button type="button" variant="ghost" onClick={onSignOut} disabled={busy}>
            Sign out
          </Button>
        </div>

        {message ? <div className="admin-notice admin-notice--danger">{message}</div> : null}

        <p className="admin-muted" style={{ fontSize: 12.5 }}>
          At least 12 characters. Other devices are signed out when the password changes.
        </p>
      </form>
    </div>
  );
}
