import React from 'react';
import { AlertTriangle, X } from 'lucide-react';

interface CloseSessionDialogProps {
  isOpen: boolean;
  uncommittedFiles: string[];
  onForceClose: () => void;
  onCancel: () => void;
  isLoading?: boolean;
}

export function CloseSessionDialog({
  isOpen,
  uncommittedFiles,
  onForceClose,
  onCancel,
  isLoading = false,
}: CloseSessionDialogProps) {
  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="w-full max-w-md rounded-lg border border-[#27272a] bg-[#09090b] shadow-xl">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-[#27272a] px-4 py-3">
          <div className="flex items-center gap-2">
            <AlertTriangle className="size-4 text-[#f59e0b]" />
            <h3 className="text-sm font-medium text-[#fafafa]">Uncommitted Changes</h3>
          </div>
          <button
            onClick={onCancel}
            className="rounded p-1 text-[#52525b] transition-colors hover:text-[#a1a1aa]"
            aria-label="Close dialog"
          >
            <X className="size-4" />
          </button>
        </div>

        {/* Body */}
        <div className="px-4 py-3 space-y-3">
          <p className="text-sm text-[#a1a1aa]">
            This session has uncommitted changes that will be lost if you force close.
          </p>

          {uncommittedFiles.length > 0 && (
            <div className="max-h-40 overflow-y-auto rounded border border-[#27272a] bg-[#111113] p-2">
              {uncommittedFiles.map((file) => (
                <div key={file} className="truncate font-mono text-xs text-[#71717a]">
                  {file}
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Actions */}
        <div className="flex items-center justify-end gap-2 border-t border-[#27272a] px-4 py-3">
          <button
            onClick={onCancel}
            disabled={isLoading}
            className="rounded-md border border-[#27272a] px-3 py-1.5 text-sm text-[#a1a1aa] transition-colors hover:border-[#3f3f46] hover:text-[#fafafa] disabled:opacity-50"
          >
            Cancel
          </button>
          <button
            onClick={onForceClose}
            disabled={isLoading}
            className="rounded-md bg-[#ef4444] px-3 py-1.5 text-sm font-medium text-white transition-colors hover:bg-[#dc2626] disabled:opacity-50"
          >
            {isLoading ? 'Closing...' : 'Force Close'}
          </button>
        </div>
      </div>
    </div>
  );
}
