import { useState } from "react";
import { Button } from "@app-directory/ui";

interface CodeCalloutProps {
  code: string;
  expiresAt: string;
  email?: string;
  purpose: "onboard" | "reset";
}

/** One-time reset/onboarding code. Shown once, with copy and expiry. */
export function CodeCallout({ code, expiresAt, email, purpose }: CodeCalloutProps) {
  const [copied, setCopied] = useState(false);

  const expiry = new Date(expiresAt).toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  });

  async function copy() {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2000);
    } catch {
      setCopied(false);
    }
  }

  return (
    <div className="admin-code">
      <div className="admin-code__label">
        {purpose === "onboard" ? "Onboarding code" : "Password reset code"}
        {email ? ` for ${email}` : ""}
      </div>
      <div className="admin-code__row">
        <code className="admin-code__value">{code}</code>
        <Button size="sm" onClick={copy}>
          {copied ? "Copied" : "Copy"}
        </Button>
      </div>
      <div className="admin-code__hint">
        Valid until {expiry}. Single use — share it with the person through a trusted channel.
      </div>
    </div>
  );
}
