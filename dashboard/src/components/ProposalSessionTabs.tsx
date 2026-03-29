import React from 'react';
import { MessageSquare, Plus, X } from 'lucide-react';
import { ProposalSession } from '../api/types';

interface ProposalSessionTabsProps {
  sessions: ProposalSession[];
  activeSessionId: string | null;
  onSelectSession: (sessionId: string) => void;
  onCreateSession: () => void;
  onCloseSession?: (sessionId: string) => void;
}

export function ProposalSessionTabs({
  sessions,
  activeSessionId,
  onSelectSession,
  onCreateSession,
  onCloseSession,
}: ProposalSessionTabsProps) {
  const activeSessions = sessions.filter((s) => s.status !== 'closed');

  if (activeSessions.length === 0) return null;

  return (
    <div className="flex items-center gap-1 border-b border-[#27272a] px-2 py-1 overflow-x-auto">
      {activeSessions.map((session) => {
        const isActive = session.id === activeSessionId;
        return (
          <button
            key={session.id}
            onClick={() => onSelectSession(session.id)}
            className={`group flex items-center gap-1.5 rounded-md px-2.5 py-1 text-xs transition-colors ${
              isActive
                ? 'bg-[#1e1b4b]/50 text-[#a5b4fc]'
                : 'text-[#52525b] hover:bg-[#27272a]/50 hover:text-[#a1a1aa]'
            }`}
          >
            <MessageSquare className="size-3" />
            <span className="max-w-[120px] truncate font-mono">
              {session.worktree_branch}
            </span>
            {session.is_dirty && (
              <span className="size-1.5 rounded-full bg-[#f59e0b]" title="Uncommitted changes" />
            )}
            {onCloseSession && (
              <span
                onClick={(e) => {
                  e.stopPropagation();
                  onCloseSession(session.id);
                }}
                className="ml-0.5 rounded p-0.5 opacity-0 transition-opacity group-hover:opacity-100 hover:bg-[#27272a]"
                role="button"
                aria-label={`Close session ${session.worktree_branch}`}
              >
                <X className="size-3" />
              </span>
            )}
          </button>
        );
      })}
      <button
        onClick={onCreateSession}
        className="flex items-center gap-1 rounded-md px-2 py-1 text-xs text-[#52525b] transition-colors hover:bg-[#27272a]/50 hover:text-[#a1a1aa]"
        aria-label="New proposal session"
      >
        <Plus className="size-3" />
      </button>
    </div>
  );
}
