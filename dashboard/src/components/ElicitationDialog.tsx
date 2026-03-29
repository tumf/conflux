import React, { useState, useCallback } from 'react';
import { X } from 'lucide-react';
import { ElicitationRequest, ElicitationProperty } from '../api/types';

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

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="w-full max-w-md rounded-lg border border-[#27272a] bg-[#09090b] shadow-xl">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-[#27272a] px-4 py-3">
          <h3 className="text-sm font-medium text-[#fafafa]">Agent Request</h3>
          <button
            onClick={onCancel}
            className="rounded p-1 text-[#52525b] transition-colors hover:text-[#a1a1aa]"
            aria-label="Close"
          >
            <X className="size-4" />
          </button>
        </div>

        {/* Message */}
        {elicitation.message && (
          <div className="border-b border-[#27272a] px-4 py-3">
            <p className="text-sm text-[#a1a1aa]">{elicitation.message}</p>
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
              className="rounded-md border border-[#27272a] px-3 py-1.5 text-sm text-[#a1a1aa] transition-colors hover:border-[#3f3f46] hover:text-[#fafafa]"
            >
              Decline
            </button>
            <button
              type="button"
              onClick={onCancel}
              className="rounded-md border border-[#27272a] px-3 py-1.5 text-sm text-[#a1a1aa] transition-colors hover:border-[#3f3f46] hover:text-[#fafafa]"
            >
              Cancel
            </button>
            <button
              type="submit"
              className="rounded-md bg-[#6366f1] px-3 py-1.5 text-sm font-medium text-white transition-colors hover:bg-[#4f46e5]"
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
          className="size-4 rounded border-[#27272a] bg-[#111113] text-[#6366f1] focus:ring-[#6366f1]"
        />
        <label htmlFor={id} className="text-sm text-[#d4d4d8]">
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
        <label htmlFor={id} className="block text-sm text-[#d4d4d8]">
          {label}
          {required && <span className="ml-1 text-[#ef4444]">*</span>}
        </label>
        {property.description && (
          <p className="text-xs text-[#52525b]">{property.description}</p>
        )}
        <select
          id={id}
          value={String(value || '')}
          onChange={(e) => onChange(e.target.value)}
          required={required}
          className="w-full rounded-md border border-[#27272a] bg-[#111113] px-3 py-2 text-sm text-[#fafafa] focus:border-[#6366f1] focus:outline-none"
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
        <label htmlFor={id} className="block text-sm text-[#d4d4d8]">
          {label}
          {required && <span className="ml-1 text-[#ef4444]">*</span>}
        </label>
        {property.description && (
          <p className="text-xs text-[#52525b]">{property.description}</p>
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
          className="w-full rounded-md border border-[#27272a] bg-[#111113] px-3 py-2 text-sm text-[#fafafa] focus:border-[#6366f1] focus:outline-none"
        />
      </div>
    );
  }

  // Default: string → text input
  return (
    <div className="space-y-1.5">
      <label htmlFor={id} className="block text-sm text-[#d4d4d8]">
        {label}
        {required && <span className="ml-1 text-[#ef4444]">*</span>}
      </label>
      {property.description && (
        <p className="text-xs text-[#52525b]">{property.description}</p>
      )}
      <input
        type="text"
        id={id}
        value={String(value || '')}
        onChange={(e) => onChange(e.target.value)}
        required={required}
        className="w-full rounded-md border border-[#27272a] bg-[#111113] px-3 py-2 text-sm text-[#fafafa] focus:border-[#6366f1] focus:outline-none"
      />
    </div>
  );
}
