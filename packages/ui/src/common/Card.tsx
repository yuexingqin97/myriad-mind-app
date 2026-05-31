// ============================================================
// Card / Modal — 通用容器组件 (纯 inline style，无 Tailwind)
// ============================================================

import React, { useEffect, useRef } from "react";

export interface CardProps {
  title?: string;
  subtitle?: string;
  icon?: React.ReactNode;
  children: React.ReactNode;
  footer?: React.ReactNode;
  className?: string;
  variant?: "default" | "bordered" | "elevated" | "accent";
  padding?: "none" | "sm" | "md" | "lg";
}

const variantMap: Record<string, React.CSSProperties> = {
  default: { background: "#1a1a2e", border: "1px solid #2a2a4a" },
  bordered: { background: "#1a1a2e", border: "2px solid rgba(99,102,241,0.5)", boxShadow: "0 1px 8px rgba(99,102,241,0.1)" },
  elevated: { background: "#1e1e36", border: "1px solid #2a2a4a", boxShadow: "0 8px 24px rgba(0,0,0,0.3)" },
  accent: { background: "#1a1a2e", border: "1px solid #2a2a4a", borderLeft: "4px solid #6366f1" },
};

const paddingMap: Record<string, React.CSSProperties> = {
  none: { padding: 0 },
  sm: { padding: 12 },
  md: { padding: 20 },
  lg: { padding: 32 },
};

export function Card({
  title,
  subtitle,
  icon,
  children,
  footer,
  className = "",
  variant = "default",
  padding = "md",
}: CardProps) {
  return (
    <div style={{ borderRadius: 12, ...variantMap[variant] }}>
      {(title || icon) && (
        <div style={{
          display: "flex", alignItems: "center", gap: 12,
          borderBottom: "1px solid #2a2a4a",
          ...paddingMap[padding],
          paddingBottom: subtitle ? 12 : 16,
        }}>
          {icon && <span style={{ fontSize: 24 }}>{icon}</span>}
          <div>
            {title && <h3 style={{ fontSize: 15, fontWeight: 600, color: "#c0a0ff", margin: 0 }}>{title}</h3>}
            {subtitle && <p style={{ fontSize: 13, color: "#a0a0c0", margin: 0, marginTop: 2 }}>{subtitle}</p>}
          </div>
        </div>
      )}
      <div style={paddingMap[padding]}>{children}</div>
      {footer && (
        <div style={{ borderTop: "1px solid #2a2a4a", ...paddingMap[padding], paddingTop: 12 }}>
          {footer}
        </div>
      )}
    </div>
  );
}

// ---- Modal ----

export interface ModalProps {
  open: boolean;
  onClose: () => void;
  title?: string;
  children: React.ReactNode;
  footer?: React.ReactNode;
  size?: "sm" | "md" | "lg";
}

const modalSizeMap: Record<string, React.CSSProperties> = {
  sm: { maxWidth: 384 },
  md: { maxWidth: 512 },
  lg: { maxWidth: 672 },
};

export function Modal({
  open,
  onClose,
  title,
  children,
  footer,
  size = "md",
}: ModalProps) {
  const overlayRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const handleKey = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    document.addEventListener("keydown", handleKey);
    document.body.style.overflow = "hidden";
    return () => { document.removeEventListener("keydown", handleKey); document.body.style.overflow = ""; };
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div
      ref={overlayRef}
      style={{
        position: "fixed", inset: 0, zIndex: 50,
        display: "flex", alignItems: "center", justifyContent: "center",
        background: "rgba(0,0,0,0.6)", backdropFilter: "blur(4px)",
      }}
      onClick={(e) => { if (e.target === overlayRef.current) onClose(); }}
    >
      <div style={{
        width: "100%", margin: "0 16px", borderRadius: 12,
        background: "#1a1a2e", boxShadow: "0 16px 48px rgba(0,0,0,0.5)",
        border: "1px solid #2a2a4a",
        ...modalSizeMap[size],
      }}>
        {title && (
          <div style={{
            display: "flex", alignItems: "center", justifyContent: "space-between",
            padding: "16px 24px", borderBottom: "1px solid #2a2a4a",
          }}>
            <h2 style={{ fontSize: 17, fontWeight: 600, color: "#e0e0f0", margin: 0 }}>{title}</h2>
            <button
              onClick={onClose}
              style={{
                padding: 6, borderRadius: 6, border: "none",
                background: "transparent", color: "#666", cursor: "pointer",
                fontSize: 16, lineHeight: 1,
              }}
            >
              ✕
            </button>
          </div>
        )}
        <div style={{ padding: "16px 24px", maxHeight: "70vh", overflowY: "auto" }}>
          {children}
        </div>
        {footer && (
          <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, padding: "16px 24px", borderTop: "1px solid #2a2a4a" }}>
            {footer}
          </div>
        )}
      </div>
    </div>
  );
}
