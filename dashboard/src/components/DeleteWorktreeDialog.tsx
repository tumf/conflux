import React from 'react';
import { AlertTriangle } from 'lucide-react';

interface DeleteWorktreeDialogProps {
  isOpen: boolean;
  branchName: string;
  onConfirm: () => void;
  onCancel: () => void;
  isLoading: boolean;
}

export function DeleteWorktreeDialog({
  isOpen,
  branchName,
  onConfirm,
  onCancel,
  isLoading,
}: DeleteWorktreeDialogProps) {
  if (!isOpen) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
      onClick={onCancel}
    >
      <div
        className="w-80 rounded-xl border border-[#27272a] bg-[#111113] p-5 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="mb-4 flex items-center gap-2.5">
          <div className="flex size-8 items-center justify-center rounded-lg bg-[#450a0a]/60">
            <AlertTriangle className="size-4 text-[#ef4444]" />
          </div>
          <h2 className="text-sm font-semibold text-[#fafafa]">Delete Worktree</h2>
        </div>

        <p className="mb-5 text-sm text-[#71717a]">
          Delete worktree <span className="font-medium font-mono text-[#fafafa]">{branchName}</span>? This cannot be undone.
        </p>

        <div className="flex gap-2">
          <button
            onClick={onCancel}
            disabled={isLoading}
            className="flex-1 rounded-lg border border-[#27272a] bg-[#18181b] px-3 py-2 text-sm text-[#a1a1aa] transition-colors hover:border-[#3f3f46] hover:text-[#fafafa] disabled:opacity-50"
          >
            Cancel
          </button>
          <button
            onClick={onConfirm}
            disabled={isLoading}
            className="flex-1 rounded-lg bg-[#ef4444] px-3 py-2 text-sm font-medium text-white transition-colors hover:bg-[#dc2626] disabled:opacity-50"
          >
            {isLoading ? 'Deleting...' : 'Delete'}
          </button>
        </div>
      </div>
    </div>
  );
}
