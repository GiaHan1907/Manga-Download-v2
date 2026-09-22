import type { InputHTMLAttributes } from "react";

export type InputProps = InputHTMLAttributes<HTMLInputElement> & {
  label: string;
  hint?: string;
};

export default function Input({ id, label, hint, ...props }: InputProps) {
  const inputId = id ?? `input-${label.toLowerCase().replace(/\s+/g, "-")}`;
  const hintId = hint ? `${inputId}-hint` : undefined;

  return (
    <label className="input-field" htmlFor={inputId}>
      <span className="input-field__label">{label}</span>
      <input {...props} id={inputId} aria-describedby={hintId} className="input" />
      {hint && (
        <span id={hintId} className="muted">
          {hint}
        </span>
      )}
    </label>
  );
}
