export function Spinner({ label }: { label?: string }) {
  return (
    <span role="status" aria-live="polite" style={{ display: "inline-flex", alignItems: "center", gap: 8 }}>
      <span className="ad-spinner" aria-hidden="true" />
      {label ? <span style={{ color: "var(--color-text-muted)" }}>{label}</span> : null}
    </span>
  );
}
