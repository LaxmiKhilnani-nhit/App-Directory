import type { ReactNode } from "react";

export interface BadgeProps {
  tone?: "neutral" | "accent" | "success" | "warning" | "danger";
  children: ReactNode;
}

export function Badge({ tone = "neutral", children }: BadgeProps) {
  const classes = tone === "neutral" ? "ad-badge" : `ad-badge ad-badge--${tone}`;
  return <span className={classes}>{children}</span>;
}
