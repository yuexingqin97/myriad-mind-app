// ============================================================
// Input — 通用表单组件 (CC Switch 暗色风格)
// ============================================================

import React from "react";

const inputBase = [
  "w-full px-3 py-2 text-sm rounded-lg border",
  "bg-[#0f0f1a]",
  "text-[#e0e0f0]",
  "placeholder:text-gray-500",
  "focus:outline-none focus:ring-2 focus:ring-offset-0",
  "transition-colors duration-150",
];

export interface InputProps
  extends React.InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  error?: string;
  hint?: string;
  icon?: React.ReactNode;
}

export function Input({
  label,
  error,
  hint,
  icon,
  className = "",
  id,
  ...props
}: InputProps) {
  const inputId = id ?? (label ? label.toLowerCase().replace(/\s+/g, "-") : undefined);

  return (
    <div className="flex flex-col gap-1.5">
      {label && (
        <label
          htmlFor={inputId}
          className="text-xs font-medium text-[#a0a0c0] uppercase tracking-wide"
        >
          {label}
        </label>
      )}
      <div className="relative">
        {icon && (
          <span className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-500">
            {icon}
          </span>
        )}
        <input
          id={inputId}
          className={[
            ...inputBase,
            error
              ? "border-red-500/70 focus:ring-red-500/50"
              : "border-[#2a2a4a] focus:border-indigo-500 focus:ring-indigo-500/40",
            icon ? "pl-10" : "",
            className,
          ].join(" ")}
          {...props}
        />
      </div>
      {error && <p className="text-xs text-red-400">{error}</p>}
      {hint && !error && (
        <p className="text-xs text-[#666]">{hint}</p>
      )}
    </div>
  );
}

export interface TextareaProps
  extends React.TextareaHTMLAttributes<HTMLTextAreaElement> {
  label?: string;
  error?: string;
}

export function Textarea({
  label,
  error,
  className = "",
  id,
  ...props
}: TextareaProps) {
  const textareaId = id ?? (label ? label.toLowerCase().replace(/\s+/g, "-") : undefined);

  return (
    <div className="flex flex-col gap-1.5">
      {label && (
        <label
          htmlFor={textareaId}
          className="text-xs font-medium text-[#a0a0c0] uppercase tracking-wide"
        >
          {label}
        </label>
      )}
      <textarea
        id={textareaId}
        className={[
          ...inputBase,
          "resize-vertical min-h-[80px]",
          error
            ? "border-red-500/70 focus:ring-red-500/50"
            : "border-[#2a2a4a] focus:border-indigo-500 focus:ring-indigo-500/40",
          className,
        ].join(" ")}
        {...props}
      />
      {error && <p className="text-xs text-red-400">{error}</p>}
    </div>
  );
}

export interface SelectOption {
  value: string;
  label: string;
}

export interface SelectProps
  extends Omit<React.SelectHTMLAttributes<HTMLSelectElement>, "children"> {
  label?: string;
  options: SelectOption[];
  placeholder?: string;
  error?: string;
}

export function Select({
  label,
  options,
  placeholder,
  error,
  className = "",
  id,
  ...props
}: SelectProps) {
  const selectId = id ?? (label ? label.toLowerCase().replace(/\s+/g, "-") : undefined);

  return (
    <div className="flex flex-col gap-1.5">
      {label && (
        <label
          htmlFor={selectId}
          className="text-xs font-medium text-[#a0a0c0] uppercase tracking-wide"
        >
          {label}
        </label>
      )}
      <select
        id={selectId}
        className={[
          ...inputBase,
          "cursor-pointer appearance-none",
          "bg-[url('data:image/svg+xml;charset=utf-8,<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"12\" height=\"12\" fill=\"%23666\"><path d=\"M6 8L1 3h10z\"/></svg>')] bg-no-repeat bg-[right_10px_center]",
          error
            ? "border-red-500/70 focus:ring-red-500/50"
            : "border-[#2a2a4a] focus:border-indigo-500 focus:ring-indigo-500/40",
          className,
        ].join(" ")}
        {...props}
      >
        {placeholder && (
          <option value="" disabled>
            {placeholder}
          </option>
        )}
        {options.map((opt) => (
          <option key={opt.value} value={opt.value}>
            {opt.label}
          </option>
        ))}
      </select>
      {error && <p className="text-xs text-red-400">{error}</p>}
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

export function Toggle({
  label,
  description,
  checked,
  onChange,
  disabled = false,
}: ToggleProps) {
  return (
    <label
      className={[
        "flex items-center gap-3 py-2",
        disabled ? "opacity-50 cursor-not-allowed" : "cursor-pointer",
      ].join(" ")}
    >
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        disabled={disabled}
        className={[
          "relative inline-flex h-5 w-9 shrink-0 rounded-full border-2 border-transparent",
          "transition-colors duration-200 focus:outline-none focus:ring-2 focus:ring-indigo-500/50 focus:ring-offset-2 focus:ring-offset-[#1a1a2e]",
          checked ? "bg-indigo-600" : "bg-[#2a2a4a]",
        ].join(" ")}
        onClick={() => onChange(!checked)}
      >
        <span
          className={[
            "pointer-events-none inline-block h-4 w-4 rounded-full bg-white shadow transform ring-0",
            "transition duration-200",
            checked ? "translate-x-4" : "translate-x-0",
          ].join(" ")}
        />
      </button>
      <div className="flex flex-col">
        <span className="text-sm font-medium text-[#ccc]">{label}</span>
        {description && (
          <span className="text-xs text-[#666]">{description}</span>
        )}
      </div>
    </label>
  );
}
