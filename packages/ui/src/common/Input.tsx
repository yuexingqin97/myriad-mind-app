// ============================================================
// Input / Select / Toggle / Textarea — 通用表单组件
// 纯 inline style，无 Tailwind 依赖
// ============================================================

import React from "react";

const inputStyle: React.CSSProperties = {
  width: "100%", padding: "8px 12px", fontSize: 13,
  borderRadius: 8, border: "1px solid #2a2a4a",
  background: "#0f0f1a", color: "#e0e0f0",
  outline: "none", transition: "border-color 0.15s",
};

export interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  error?: string;
  hint?: string;
  icon?: React.ReactNode;
}

export function Input({ label, error, hint, icon, id, className = "", ...props }: InputProps) {
  const inputId = id ?? (label ? label.toLowerCase().replace(/\s+/g, "-") : undefined);
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      {label && (
        <label htmlFor={inputId} style={{ fontSize: 11, fontWeight: 600, color: "#a0a0c0", textTransform: "uppercase", letterSpacing: "0.05em" }}>
          {label}
        </label>
      )}
      <div style={{ position: "relative" }}>
        {icon && <span style={{ position: "absolute", left: 12, top: "50%", transform: "translateY(-50%)", color: "#666" }}>{icon}</span>}
        <input
          id={inputId}
          style={{
            ...inputStyle,
            borderColor: error ? "rgba(248,113,113,0.7)" : "#2a2a4a",
            paddingLeft: icon ? 36 : 12,
          }}
          {...props}
        />
      </div>
      {error && <p style={{ fontSize: 11, color: "#f87171", margin: 0 }}>{error}</p>}
      {hint && !error && <p style={{ fontSize: 11, color: "#666", margin: 0 }}>{hint}</p>}
    </div>
  );
}

export interface TextareaProps extends React.TextareaHTMLAttributes<HTMLTextAreaElement> {
  label?: string;
  error?: string;
}

export function Textarea({ label, error, id, className = "", ...props }: TextareaProps) {
  const textareaId = id ?? (label ? label.toLowerCase().replace(/\s+/g, "-") : undefined);
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      {label && (
        <label htmlFor={textareaId} style={{ fontSize: 11, fontWeight: 600, color: "#a0a0c0", textTransform: "uppercase", letterSpacing: "0.05em" }}>
          {label}
        </label>
      )}
      <textarea
        id={textareaId}
        style={{ ...inputStyle, resize: "vertical", minHeight: 80, borderColor: error ? "rgba(248,113,113,0.7)" : "#2a2a4a" }}
        {...props}
      />
      {error && <p style={{ fontSize: 11, color: "#f87171", margin: 0 }}>{error}</p>}
    </div>
  );
}

export interface SelectOption { value: string; label: string; }

export interface SelectProps extends Omit<React.SelectHTMLAttributes<HTMLSelectElement>, "children"> {
  label?: string;
  options: SelectOption[];
  placeholder?: string;
  error?: string;
}

export function Select({ label, options, placeholder, error, id, className = "", ...props }: SelectProps) {
  const selectId = id ?? (label ? label.toLowerCase().replace(/\s+/g, "-") : undefined);
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      {label && (
        <label htmlFor={selectId} style={{ fontSize: 11, fontWeight: 600, color: "#a0a0c0", textTransform: "uppercase", letterSpacing: "0.05em" }}>
          {label}
        </label>
      )}
      <select
        id={selectId}
        style={{
          ...inputStyle, cursor: "pointer",
          appearance: "none",
          backgroundImage: "url(\"data:image/svg+xml;charset=utf-8,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' fill='%23666'%3E%3Cpath d='M6 8L1 3h10z'/%3E%3C/svg%3E\")",
          backgroundRepeat: "no-repeat", backgroundPosition: "right 10px center",
          paddingRight: 28,
          borderColor: error ? "rgba(248,113,113,0.7)" : "#2a2a4a",
        }}
        {...props}
      >
        {placeholder && <option value="" disabled>{placeholder}</option>}
        {options.map((opt) => <option key={opt.value} value={opt.value}>{opt.label}</option>)}
      </select>
      {error && <p style={{ fontSize: 11, color: "#f87171", margin: 0 }}>{error}</p>}
    </div>
  );
}

export interface ToggleProps {
  label: string;
  description?: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}

export function Toggle({ label, description, checked, onChange, disabled = false }: ToggleProps) {
  return (
    <label style={{
      display: "flex", alignItems: "flex-start", gap: 12, padding: "6px 0",
      cursor: disabled ? "not-allowed" : "pointer", opacity: disabled ? 0.5 : 1,
    }}>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        disabled={disabled}
        onClick={() => !disabled && onChange(!checked)}
        style={{
          position: "relative", display: "inline-flex",
          height: 20, width: 36, flexShrink: 0,
          borderRadius: 10, border: "2px solid transparent",
          background: checked ? "#6366f1" : "#2a2a4a",
          transition: "background 0.2s", outline: "none",
          cursor: disabled ? "not-allowed" : "pointer",
          marginTop: 2,
        }}
      >
        <span style={{
          position: "absolute", top: 2,
          display: "block", height: 16, width: 16, borderRadius: "50%",
          background: "#fff", boxShadow: "0 1px 3px rgba(0,0,0,0.3)",
          transition: "left 0.2s",
          left: checked ? 18 : 2,
        }} />
      </button>
      <div style={{ display: "flex", flexDirection: "column" }}>
        <span style={{ fontSize: 13, fontWeight: 500, color: "#ccc" }}>{label}</span>
        {description && <span style={{ fontSize: 11, color: "#666", marginTop: 2 }}>{description}</span>}
      </div>
    </label>
  );
}
