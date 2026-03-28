import React from 'react';
import { Plus, RefreshCw } from 'lucide-react';
import { WorktreeInfo } from '../api/types';
import { WorktreeRow } from './WorktreeRow';

interface WorktreesPanelProps {
  worktrees: WorktreeInfo[];
  selectedProjectId: string | null;
  onMerge: (branchName: string) => void;
  onDelete: (branchName: string) => void;
  onCreate: () => void;
  onRefresh: () => void;
  onClickWorktree?: (branch: string) => void;
  selectedWorktreeBranch?: string | null;
  isLoading: boolean;
}

export function WorktreesPanel({
  worktrees,
  selectedProjectId,
  onMerge,
  onDelete,
  onCreate,
  onRefresh,
  onClickWorktree,
  selectedWorktreeBranch,
  isLoading,
}: WorktreesPanelProps) {
  if (!selectedProjectId) {
    return (
      <div className="flex flex-1 items-center justify-center p-8">
        <p className="text-sm text-[#52525b]">Select a project to view worktrees</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col">
      <div className="flex items-center justify-between border-b border-[#27272a] px-3 py-2">
        <span className="text-xs font-medium text-[#52525b] uppercase tracking-wider">
          Worktrees ({worktrees.length})
        </span>
        <div className="flex items-center gap-1">
          <button
            onClick={onRefresh}
            disabled={isLoading}
            title="Refresh worktrees"
            className="rounded p-1 text-[#52525b] transition-colors hover:bg-[#27272a] hover:text-[#a1a1aa] disabled:opacity-50"
          >
            <RefreshCw className="size-3.5" />
          </button>
          <button
            onClick={onCreate}
            disabled={isLoading}
            title="Create worktree"
            className="rounded p-1 text-[#52525b] transition-colors hover:bg-[#27272a] hover:text-[#6366f1] disabled:opacity-50"
          >
            <Plus className="size-3.5" />
          </button>
        </div>
      </div>

      {worktrees.length === 0 ? (
        <div className="flex flex-1 items-center justify-center p-8">
          <p className="text-sm text-[#52525b]">No worktrees</p>
        </div>
      ) : (
        <div className="space-y-1.5 p-3">
          {[...worktrees].sort((a, b) => (a.is_main === b.is_main ? 0 : a.is_main ? -1 : 1)).map((wt) => (
            <WorktreeRow
              key={wt.branch || wt.path}
              worktree={wt}
              onMerge={onMerge}
              onDelete={onDelete}
              onClickWorktree={onClickWorktree}
              isSelected={selectedWorktreeBranch === wt.branch}
              isLoading={isLoading}
            />
          ))}
        </div>
      )}
    </div>
  );
}
