import React from 'react';
import { GitMerge, X } from 'lucide-react';
import { ProposalSession } from '../api/types';

interface ProposalActionsProps {
  session: ProposalSession;
  onMerge: () => void;
  onClose: () => void;
  isLoading?: boolean;
}

export function ProposalActions({ session, onMerge, onClose, isLoading = false }: ProposalActionsProps) {
  const canMerge = session.status === 'active' && !session.is_dirty;
  const isClosed = session.status === 'closed';

  if (isClosed) {
    return null;
  }

  return (
    <div className="flex items-center gap-2">
      <button
        onClick={onMerge}
        disabled={!canMerge || isLoading}
        title={session.is_dirty ? 'Cannot merge: worktree has uncommitted changes' : 'Merge to base branch'}
        className="flex items-center gap-1.5 rounded-md bg-[#22c55e]/10 px-2.5 py-1.5 text-xs font-medium text-[#22c55e] transition-colors hover:bg-[#22c55e]/20 disabled:cursor-not-allowed disabled:opacity-40"
      >
        <GitMerge className="size-3.5" />
        Merge
      </button>
      <button
        onClick={onClose}
        disabled={isLoading}
        className="flex items-center gap-1.5 rounded-md border border-[#27272a] px-2.5 py-1.5 text-xs font-medium text-[#a1a1aa] transition-colors hover:border-[#3f3f46] hover:text-[#fafafa] disabled:opacity-40"
      >
        <X className="size-3.5" />
        Close
      </button>
    </div>
  );
}
