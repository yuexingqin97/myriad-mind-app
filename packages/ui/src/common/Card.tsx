// ============================================================
// Card / Modal — 通用容器组件 (CC Switch 暗色风格)
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

const variantStyles = {
  default:
    "bg-[#1a1a2e] border border-[#2a2a4a]",
  bordered:
    "bg-[#1a1a2e] border-2 border-indigo-500/60 shadow-sm shadow-indigo-500/10",
  elevated:
    "bg-[#1e1e36] border border-[#2a2a4a] shadow-lg shadow-black/30",
  accent:
    "bg-[#1a1a2e] border-l-4 border-indigo-500 border border-[#2a2a4a] border-l-indigo-500",
};

const paddingStyles = {
  none: "p-0",
  sm: "p-3",
  md: "p-5",
  lg: "p-8",
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
    <div
      className={[
        "rounded-xl",
        variantStyles[variant],
        className,
      ].join(" ")}
    >
      {(title || icon) && (
        <div
          className={[
            "flex items-center gap-3 border-b border-[#2a2a4a]",
            paddingStyles[padding],
            subtitle ? "pb-3" : "pb-4",
          ].join(" ")}
        >
          {icon && (
            <span className="text-2xl">{icon}</span>
          )}
          <div>
            {title && (
              <h3 className="text-base font-semibold text-[#c0a0ff]">
                {title}
              </h3>
            )}
            {subtitle && (
              <p className="text-sm text-[#a0a0c0] mt-0.5">
                {subtitle}
              </p>
            )}
          </div>
        </div>
      )}
      <div className={paddingStyles[padding]}>{children}</div>
      {footer && (
        <div
          className={[
            "border-t border-[#2a2a4a]",
            paddingStyles[padding],
            "pt-3",
          ].join(" ")}
        >
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

const modalSizeStyles = {
  sm: "max-w-sm",
  md: "max-w-lg",
  lg: "max-w-2xl",
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
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handleKey);
    document.body.style.overflow = "hidden";
    return () => {
      document.removeEventListener("keydown", handleKey);
      document.body.style.overflow = "";
    };
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div
      ref={overlayRef}
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
      onClick={(e) => {
        if (e.target === overlayRef.current) onClose();
      }}
    >
      <div
        className={[
          "w-full mx-4 rounded-xl bg-[#1a1a2e] shadow-2xl shadow-black/50",
          "border border-[#2a2a4a]",
          modalSizeStyles[size],
        ].join(" ")}
      >
        {title && (
          <div className="flex items-center justify-between px-6 py-4 border-b border-[#2a2a4a]">
            <h2 className="text-lg font-semibold text-[#e0e0f0]">
              {title}
            </h2>
            <button
              onClick={onClose}
              className="p-1.5 rounded-md text-gray-500 hover:text-gray-300 hover:bg-white/5 transition-colors"
            >
              <svg
                className="w-5 h-5"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M6 18L18 6M6 6l12 12"
                />
              </svg>
            </button>
          </div>
        )}
        <div className="px-6 py-4 max-h-[70vh] overflow-y-auto">
          {children}
        </div>
        {footer && (
          <div className="flex justify-end gap-2 px-6 py-4 border-t border-[#2a2a4a]">
            {footer}
          </div>
        )}
      </div>
    </div>
  );
}
