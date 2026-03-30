import React, { useState, useCallback, useEffect, useMemo, useRef } from 'react';
import { X } from 'lucide-react';
import { ElicitationRequest, ElicitationProperty } from '../api/types';

const FOCUSABLE_SELECTOR = [
  'button:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  'a[href]',
  '[tabindex]:not([tabindex="-1"])',
].join(', ');

function getFocusableElements(container: HTMLElement): HTMLElement[] {
  return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR));
}

interface ElicitationDialogProps {
  elicitation: ElicitationRequest;
  onSubmit: (data: Record<string, unknown>) => void;
  onDecline: () => void;
  onCancel: () => void;
}

export function ElicitationDialog({ elicitation, onSubmit, onDecline, onCancel }: ElicitationDialogProps) {
  const [formData, setFormData] = useState<Record<string, unknown>>(() => {
    const initial: Record<string, unknown> = {};
    for (const [key, prop] of Object.entries(elicitation.properties)) {
      if (prop.default !== undefined) {
        initial[key] = prop.default;
      } else if (prop.type === 'boolean') {
        initial[key] = false;
      } else if (prop.type === 'number' || prop.type === 'integer') {
        initial[key] = 0;
      } else {
        initial[key] = '';
      }
    }
    return initial;
  });

  const dialogId = useMemo(() => `elicitation-dialog-${elicitation.id}`, [elicitation.id]);
  const titleId = `${dialogId}-title`;
  const dialogRef = useRef<HTMLDivElement>(null);

  const handleChange = useCallback((key: string, value: unknown) => {
    setFormData((prev) => ({ ...prev, [key]: value }));
  }, []);

  const handleSubmit = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      onSubmit(formData);
    },
    [formData, onSubmit],
  );

  const handleBackdropClick = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      if (e.target === e.currentTarget) {
        onCancel();
      }
    },
    [onCancel],
  );

  const handleDialogKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLDivElement>) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        onCancel();
        return;
      }

      if (e.key !== 'Tab') {
        return;
      }

      const container = dialogRef.current;
      if (!container) {
        return;
      }

      const focusableElements = getFocusableElements(container);
      if (focusableElements.length === 0) {
        e.preventDefault();
        container.focus();
        return;
      }

      const first = focusableElements[0];
      const last = focusableElements[focusableElements.length - 1];
      const active = document.activeElement as HTMLElement | null;

      if (e.shiftKey) {
        if (active === first || !container.contains(active)) {
          e.preventDefault();
          last.focus();
        }
      } else if (active === last || !container.contains(active)) {
        e.preventDefault();
        first.focus();
      }
    },
    [onCancel],
  );

  useEffect(() => {
    const container = dialogRef.current;
    if (!container) {
      return;
    }

    const focusableElements = getFocusableElements(container);
    if (focusableElements.length > 0) {
      focusableElements[0].focus();
    } else {
      container.focus();
    }
  }, []);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
      onClick={handleBackdropClick}
    >
      <div
        ref={dialogRef}
        className="w-full max-w-md rounded-lg border border-border bg-bg shadow-xl"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        id={dialogId}
        tabIndex={-1}
        onKeyDown={handleDialogKeyDown}
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-border px-4 py-3">
          <h3 id={titleId} className="text-sm font-medium text-text">Agent Request</h3>
          <button
            onClick={onCancel}
            className="rounded p-1 text-text-subtle transition-colors hover:text-text-muted"
            aria-label="Close"
          >
            <X className="size-4" />
          </button>
        </div>

        {/* Message */}
        {elicitation.message && (
          <div className="border-b border-border px-4 py-3">
            <p className="text-sm text-text-muted">{elicitation.message}</p>
          </div>
        )}

        {/* Form */}
        <form onSubmit={handleSubmit} className="space-y-4 px-4 py-4">
          {Object.entries(elicitation.properties).map(([key, prop]) => (
            <FormField
              key={key}
              name={key}
              property={prop}
              value={formData[key]}
              onChange={(val) => handleChange(key, val)}
              required={elicitation.required?.includes(key)}
            />
          ))}

          {/* Actions */}
          <div className="flex items-center justify-end gap-2 pt-2">
            <button
              type="button"
              onClick={onDecline}
              className="rounded-md border border-border px-3 py-1.5 text-sm text-text-muted transition-colors hover:border-border-hover hover:text-text"
            >
              Decline
            </button>
            <button
              type="button"
              onClick={onCancel}
              className="rounded-md border border-border px-3 py-1.5 text-sm text-text-muted transition-colors hover:border-border-hover hover:text-text"
            >
              Cancel
            </button>
            <button
              type="submit"
              className="rounded-md bg-accent px-3 py-1.5 text-sm font-medium text-white transition-colors hover:bg-accent-hover"
            >
              Submit
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

interface FormFieldProps {
  name: string;
  property: ElicitationProperty;
  value: unknown;
  onChange: (value: unknown) => void;
  required?: boolean;
}

function FormField({ name, property, value, onChange, required }: FormFieldProps) {
  const label = property.title || name;
  const id = `elicitation-${name}`;

  // Boolean → checkbox
  if (property.type === 'boolean') {
    return (
      <div className="flex items-center gap-2">
        <input
          type="checkbox"
          id={id}
          checked={Boolean(value)}
          onChange={(e) => onChange(e.target.checked)}
          className="size-4 rounded border-border bg-surface text-accent focus:ring-accent"
        />
        <label htmlFor={id} className="text-sm text-text">
          {label}
        </label>
      </div>
    );
  }

  // String with oneOf or enum → select
  if (property.type === 'string' && (property.oneOf || property.enum)) {
    const options = property.oneOf
      ? property.oneOf.map((o) => ({ value: o.const, label: o.title }))
      : (property.enum || []).map((e) => ({ value: e, label: e }));

    return (
      <div className="space-y-1.5">
        <label htmlFor={id} className="block text-sm text-text">
          {label}
          {required && <span className="ml-1 text-error">*</span>}
        </label>
        {property.description && (
          <p className="text-xs text-text-subtle">{property.description}</p>
        )}
        <select
          id={id}
          value={String(value || '')}
          onChange={(e) => onChange(e.target.value)}
          required={required}
          className="w-full rounded-md border border-border bg-surface px-3 py-2 text-sm text-text focus:border-accent focus:outline-none"
        >
          <option value="">Select...</option>
          {options.map((opt) => (
            <option key={opt.value} value={opt.value}>
              {opt.label}
            </option>
          ))}
        </select>
      </div>
    );
  }

  // Number / integer → number input
  if (property.type === 'number' || property.type === 'integer') {
    return (
      <div className="space-y-1.5">
        <label htmlFor={id} className="block text-sm text-text">
          {label}
          {required && <span className="ml-1 text-error">*</span>}
        </label>
        {property.description && (
          <p className="text-xs text-text-subtle">{property.description}</p>
        )}
        <input
          type="number"
          id={id}
          value={Number(value) || 0}
          onChange={(e) => {
            const num = property.type === 'integer'
              ? parseInt(e.target.value, 10)
              : parseFloat(e.target.value);
            onChange(isNaN(num) ? 0 : num);
          }}
          step={property.type === 'integer' ? 1 : 'any'}
          required={required}
          className="w-full rounded-md border border-border bg-surface px-3 py-2 text-sm text-text focus:border-accent focus:outline-none"
        />
      </div>
    );
  }

  // Default: string → text input
  return (
    <div className="space-y-1.5">
      <label htmlFor={id} className="block text-sm text-text">
        {label}
        {required && <span className="ml-1 text-error">*</span>}
      </label>
      {property.description && (
        <p className="text-xs text-text-subtle">{property.description}</p>
      )}
      <input
        type="text"
        id={id}
        value={String(value || '')}
        onChange={(e) => onChange(e.target.value)}
        required={required}
        className="w-full rounded-md border border-border bg-surface px-3 py-2 text-sm text-text focus:border-accent focus:outline-none"
      />
    </div>
  );
}
