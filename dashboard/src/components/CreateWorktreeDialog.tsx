import React, { useState } from 'react';
import { Plus } from 'lucide-react';

interface CreateWorktreeDialogProps {
  isOpen: boolean;
  onSubmit: (changeId: string) => void;
  onCancel: () => void;
  isLoading: boolean;
}

export function CreateWorktreeDialog({
  isOpen,
  onSubmit,
  onCancel,
  isLoading,
}: CreateWorktreeDialogProps) {
  const [changeId, setChangeId] = useState('');

  if (!isOpen) return null;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!changeId.trim()) return;
    onSubmit(changeId.trim());
  };

  const handleCancel = () => {
    setChangeId('');
    onCancel();
  };

  const isValid = changeId.trim() !== '';

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
      onClick={handleCancel}
    >
      <div
        className="w-96 rounded-xl border border-[#27272a] bg-[#111113] p-5 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="mb-4 flex items-center gap-2.5">
          <div className="flex size-8 items-center justify-center rounded-lg bg-[#1e1b4b]/60">
            <Plus className="size-4 text-[#6366f1]" />
          </div>
          <h2 className="text-sm font-semibold text-[#fafafa]">Create Worktree</h2>
        </div>

        <form onSubmit={handleSubmit}>
          <div className="mb-5">
            <label htmlFor="change-id" className="mb-1 block text-xs font-medium text-[#71717a]">
              Change ID
            </label>
            <input
              id="change-id"
              type="text"
              value={changeId}
              onChange={(e) => setChangeId(e.target.value)}
              placeholder="add-new-feature"
              disabled={isLoading}
              autoFocus
              className="w-full rounded-lg border border-[#27272a] bg-[#18181b] px-3 py-2 text-sm text-[#fafafa] placeholder-[#52525b] outline-none transition-colors focus:border-[#6366f1] disabled:opacity-50"
            />
          </div>

          <div className="flex gap-2">
            <button
              type="button"
              onClick={handleCancel}
              disabled={isLoading}
              className="flex-1 rounded-lg border border-[#27272a] bg-[#18181b] px-3 py-2 text-sm text-[#a1a1aa] transition-colors hover:border-[#3f3f46] hover:text-[#fafafa] disabled:opacity-50"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={isLoading || !isValid}
              className="flex-1 rounded-lg bg-[#6366f1] px-3 py-2 text-sm font-medium text-white transition-colors hover:bg-[#4f46e5] disabled:opacity-50"
            >
              {isLoading ? 'Creating...' : 'Create'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
