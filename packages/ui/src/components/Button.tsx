import type { ButtonHTMLAttributes, ReactNode } from "react";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "secondary" | "ghost" | "danger";
  size?: "md" | "sm";
  children?: ReactNode;
}

export function Button({
  variant = "secondary",
  size = "md",
  type = "button",
  className,
  children,
  ...rest
}: ButtonProps) {
  const classes = ["ad-btn", `ad-btn--${variant}`, size === "sm" ? "ad-btn--sm" : null, className]
    .filter(Boolean)
    .join(" ");

  return (
    <button type={type} className={classes} {...rest}>
      {children}
    </button>
  );
}
