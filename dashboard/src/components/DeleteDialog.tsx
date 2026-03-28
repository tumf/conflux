/**
 * DeleteDialog Component
 * Alert dialog for confirming project deletion
 */

import React from 'react';
import { AlertTriangle } from 'lucide-react';

interface DeleteDialogProps {
  isOpen: boolean;
  projectName: string;
  onConfirm: () => void;
  onCancel: () => void;
  isLoading: boolean;
}

export function DeleteDialog({
  isOpen,
  projectName,
  onConfirm,
  onCancel,
  isLoading,
}: DeleteDialogProps) {
  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="w-96 rounded-lg bg-color-surface p-6">
        <div className="mb-4 flex items-center gap-3">
          <AlertTriangle className="h-6 w-6 text-color-warning" />
          <h2 className="text-lg font-bold text-color-text">Delete Project</h2>
        </div>

        <p className="mb-6 text-color-text-secondary">
          Are you sure you want to delete <strong>{projectName}</strong>? This action cannot be undone.
        </p>

        <div className="flex gap-3">
          <button
            onClick={onCancel}
            disabled={isLoading}
            className="flex-1 rounded bg-color-border px-4 py-2 text-color-text hover:bg-color-border/80 disabled:opacity-50"
          >
            Cancel
          </button>
          <button
            onClick={onConfirm}
            disabled={isLoading}
            className="flex-1 rounded bg-color-error px-4 py-2 text-white hover:bg-red-700 disabled:opacity-50"
          >
            {isLoading ? 'Deleting...' : 'Delete'}
          </button>
        </div>
      </div>
    </div>
  );
}
