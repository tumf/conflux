import React, { useState } from 'react';
import { Plus } from 'lucide-react';

interface AddProjectDialogProps {
  isOpen: boolean;
  onSubmit: (remoteUrl: string, branch: string) => void;
  onCancel: () => void;
  isLoading: boolean;
}

export function AddProjectDialog({
  isOpen,
  onSubmit,
  onCancel,
  isLoading,
}: AddProjectDialogProps) {
  const [remoteUrl, setRemoteUrl] = useState('');
  const [branch, setBranch] = useState('');

  if (!isOpen) return null;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!remoteUrl.trim() || !branch.trim()) return;
    onSubmit(remoteUrl.trim(), branch.trim());
  };

  const handleCancel = () => {
    setRemoteUrl('');
    setBranch('');
    onCancel();
  };

  const isValid = remoteUrl.trim() !== '' && branch.trim() !== '';

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
          <h2 className="text-sm font-semibold text-[#fafafa]">Add Project</h2>
        </div>

        <form onSubmit={handleSubmit}>
          <div className="mb-3">
            <label htmlFor="remote-url" className="mb-1 block text-xs font-medium text-[#71717a]">
              Remote URL
            </label>
            <input
              id="remote-url"
              type="text"
              value={remoteUrl}
              onChange={(e) => setRemoteUrl(e.target.value)}
              placeholder="https://github.com/user/repo.git"
              disabled={isLoading}
              className="w-full rounded-lg border border-[#27272a] bg-[#18181b] px-3 py-2 text-sm text-[#fafafa] placeholder-[#52525b] outline-none transition-colors focus:border-[#6366f1] disabled:opacity-50"
            />
          </div>

          <div className="mb-5">
            <label htmlFor="branch" className="mb-1 block text-xs font-medium text-[#71717a]">
              Branch
            </label>
            <input
              id="branch"
              type="text"
              value={branch}
              onChange={(e) => setBranch(e.target.value)}
              placeholder="main"
              disabled={isLoading}
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
              {isLoading ? 'Adding…' : 'Add Project'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
