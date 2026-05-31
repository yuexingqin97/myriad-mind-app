// ============================================================
// Button — 通用按钮组件 (纯 inline style，无 Tailwind)
// ============================================================

import React from "react";

export type ButtonVariant = "primary" | "secondary" | "danger" | "ghost";
export type ButtonSize = "sm" | "md" | "lg";

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  loading?: boolean;
  icon?: React.ReactNode;
}

const variantMap: Record<ButtonVariant, React.CSSProperties> = {
  primary: { background: "#6366f1", color: "#fff", border: "none", boxShadow: "0 1px 4px rgba(99,102,241,0.3)" },
  secondary: { background: "#1f2937", color: "#e5e7eb", border: "1px solid #374151" },
  danger: { background: "rgba(239,68,68,0.9)", color: "#fff", border: "none", boxShadow: "0 1px 4px rgba(239,68,68,0.2)" },
  ghost: { background: "transparent", color: "#9ca3af", border: "none" },
};

const sizeMap: Record<ButtonSize, React.CSSProperties> = {
  sm: { padding: "6px 10px", fontSize: 11, borderRadius: 6 },
  md: { padding: "8px 16px", fontSize: 13, borderRadius: 8 },
  lg: { padding: "12px 24px", fontSize: 15, borderRadius: 10 },
};

export function Button({
  variant = "primary",
  size = "md",
  loading = false,
  icon,
  children,
  className = "",
  disabled,
  ...props
}: ButtonProps) {
  return (
    <button
      style={{
        display: "inline-flex", alignItems: "center", justifyContent: "center", gap: 8,
        fontWeight: 500, cursor: disabled || loading ? "not-allowed" : "pointer",
        opacity: disabled || loading ? 0.5 : 1,
        transition: "all 0.15s",
        outline: "none",
        ...variantMap[variant],
        ...sizeMap[variant === "ghost" ? size : size],
      }}
      disabled={disabled || loading}
      {...props}
    >
      {loading ? <Spinner /> : icon}
      {children}
    </button>
  );
}

function Spinner() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" style={{ animation: "spin 1s linear infinite" }}>
      <circle cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" opacity={0.25} />
      <path fill="currentColor" opacity={0.75} d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
    </svg>
  );
}
