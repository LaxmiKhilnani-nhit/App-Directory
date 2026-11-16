import type {
  InputHTMLAttributes,
  ReactNode,
  SelectHTMLAttributes,
  TextareaHTMLAttributes,
} from "react";

interface FieldShellProps {
  label: string;
  hint?: string;
  htmlFor?: string;
  children: ReactNode;
}

export function Field({ label, hint, htmlFor, children }: FieldShellProps) {
  return (
    <div className="admin-field">
      <label htmlFor={htmlFor}>{label}</label>
      {children}
      {hint ? <span className="admin-field__hint">{hint}</span> : null}
    </div>
  );
}

interface TextFieldProps extends InputHTMLAttributes<HTMLInputElement> {
  label: string;
  hint?: string;
}

export function TextField({ label, hint, id, ...rest }: TextFieldProps) {
  const inputId = id ?? `field-${label.toLowerCase().replace(/[^a-z0-9]+/g, "-")}`;
  return (
    <Field label={label} hint={hint} htmlFor={inputId}>
      <input id={inputId} {...rest} />
    </Field>
  );
}

interface SelectFieldProps extends SelectHTMLAttributes<HTMLSelectElement> {
  label: string;
  hint?: string;
}

export function SelectField({ label, hint, id, children, ...rest }: SelectFieldProps) {
  const inputId = id ?? `field-${label.toLowerCase().replace(/[^a-z0-9]+/g, "-")}`;
  return (
    <Field label={label} hint={hint} htmlFor={inputId}>
      <select id={inputId} {...rest}>
        {children}
      </select>
    </Field>
  );
}

interface TextAreaFieldProps extends TextareaHTMLAttributes<HTMLTextAreaElement> {
  label: string;
  hint?: string;
}

export function TextAreaField({ label, hint, id, ...rest }: TextAreaFieldProps) {
  const inputId = id ?? `field-${label.toLowerCase().replace(/[^a-z0-9]+/g, "-")}`;
  return (
    <Field label={label} hint={hint} htmlFor={inputId}>
      <textarea id={inputId} rows={3} {...rest} />
    </Field>
  );
}

interface CheckboxFieldProps {
  label: string;
  hint?: string;
  checked: boolean;
  disabled?: boolean;
  onChange: (checked: boolean) => void;
}

export function CheckboxField({ label, hint, checked, disabled, onChange }: CheckboxFieldProps) {
  return (
    <label className="admin-checkbox">
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(event) => onChange(event.target.checked)}
      />
      <span>
        <span className="admin-checkbox__label">{label}</span>
        {hint ? <span className="admin-checkbox__hint">{hint}</span> : null}
      </span>
    </label>
  );
}
