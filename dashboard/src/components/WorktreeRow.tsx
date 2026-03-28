import React from 'react';
import { GitBranch, GitMerge, Trash2 } from 'lucide-react';
import { WorktreeInfo } from '../api/types';

interface WorktreeRowProps {
  worktree: WorktreeInfo;
  onMerge?: (branchName: string) => void;
  onDelete?: (branchName: string) => void;
  onClickWorktree?: (branch: string) => void;
  isSelected?: boolean;
  isLoading?: boolean;
}

export function WorktreeRow({ worktree, onMerge, onDelete, onClickWorktree, isSelected, isLoading }: WorktreeRowProps) {
  const canMerge =
    !worktree.is_main &&
    !worktree.is_detached &&
    worktree.has_commits_ahead &&
    !worktree.merge_conflict &&
    !worktree.is_merging;

  const canDelete = !worktree.is_main;

  const label = worktree.branch || worktree.head;

  const handleRowClick = () => {
    if (!worktree.is_main) {
      onClickWorktree?.(worktree.branch);
    }
  };

  return (
    <div
      onClick={handleRowClick}
      className={`flex items-center justify-between gap-2 rounded-md border p-3 transition-colors ${
        isSelected
          ? 'border-[#6366f1] bg-[#1e1b4b]/30'
          : 'border-[#27272a] bg-[#111113] hover:border-[#3f3f46]'
      } ${!worktree.is_main ? 'cursor-pointer' : ''}`}
    >
      <div className="flex min-w-0 flex-1 items-center gap-2">
        <GitBranch className="size-3.5 shrink-0 text-[#52525b]" />
        <span className="truncate font-mono text-xs text-[#a1a1aa]">{label}</span>

        <div className="flex shrink-0 items-center gap-1">
          {worktree.is_main && (
            <span className="rounded px-1.5 py-0.5 text-xs font-medium text-[#6366f1] bg-[#1e1b4b]/50">
              MAIN
            </span>
          )}
          {worktree.is_detached && (
            <span className="rounded px-1.5 py-0.5 text-xs font-medium text-[#71717a] bg-[#27272a]">
              DETACHED
            </span>
          )}
          {worktree.is_merging && (
            <span className="rounded px-1.5 py-0.5 text-xs font-medium text-[#f59e0b] bg-[#451a03]/50">
              merging
            </span>
          )}
          {worktree.has_commits_ahead && !worktree.is_merging && !worktree.merge_conflict && (
            <span className="rounded px-1.5 py-0.5 text-xs font-medium text-[#22c55e] bg-[#052e16]/50">
              ahead
            </span>
          )}
          {worktree.merge_conflict && (
            <span className="rounded px-1.5 py-0.5 text-xs font-medium text-[#ef4444] bg-[#450a0a]/50">
              {worktree.merge_conflict.conflict_files.length} conflict{worktree.merge_conflict.conflict_files.length !== 1 ? 's' : ''}
            </span>
          )}
        </div>
      </div>

      <div className="flex shrink-0 items-center gap-1">
        {canMerge && onMerge && (
          <button
            onClick={() => onMerge(worktree.branch)}
            disabled={isLoading}
            title="Merge branch"
            className="rounded p-1.5 text-[#52525b] transition-colors hover:bg-[#27272a] hover:text-[#22c55e] disabled:opacity-50"
          >
            <GitMerge className="size-3.5" />
          </button>
        )}
        {canDelete && onDelete && (
          <button
            onClick={() => onDelete(worktree.branch)}
            disabled={isLoading}
            title="Delete worktree"
            className="rounded p-1.5 text-[#52525b] transition-colors hover:bg-[#27272a] hover:text-[#ef4444] disabled:opacity-50"
          >
            <Trash2 className="size-3.5" />
          </button>
        )}
      </div>
    </div>
  );
}
