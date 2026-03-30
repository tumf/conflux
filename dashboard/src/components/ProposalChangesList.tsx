import React, { useState, useEffect, useCallback } from 'react';
import { FileText, Loader2 } from 'lucide-react';
import { ProposalSessionChange } from '../api/types';
import { listProposalSessionChanges } from '../api/restClient';

interface ProposalChangesListProps {
  projectId: string;
  sessionId: string;
  onClickChange?: (changeId: string) => void;
}

export function ProposalChangesList({ projectId, sessionId, onClickChange }: ProposalChangesListProps) {
  const [changes, setChanges] = useState<ProposalSessionChange[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchChanges = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const result = await listProposalSessionChanges(projectId, sessionId);
      setChanges(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsLoading(false);
    }
  }, [projectId, sessionId]);

  // Fetch on mount and periodically (every 10s while session active)
  useEffect(() => {
    fetchChanges();
    const interval = setInterval(fetchChanges, 10000);
    return () => clearInterval(interval);
  }, [fetchChanges]);

  if (isLoading && changes.length === 0) {
    return (
      <div className="flex items-center justify-center p-4">
        <Loader2 className="size-4 animate-spin text-text-subtle" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-3">
        <p className="text-xs text-error">{error}</p>
      </div>
    );
  }

  if (changes.length === 0) {
    return (
      <div className="flex items-center justify-center p-4">
        <p className="text-xs text-text-subtle">No changes detected yet</p>
      </div>
    );
  }

  return (
    <div className="space-y-1 p-2">
      <div className="px-1 py-1.5">
        <span className="text-xs font-medium text-text-subtle uppercase tracking-wider">Changes</span>
      </div>
      {changes.map((change) => (
        <button
          key={change.change_id}
          onClick={() => onClickChange?.(change.change_id)}
          className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs text-text-muted transition-colors hover:bg-border/50"
        >
          <FileText className="size-3.5 shrink-0 text-accent" />
          <div className="min-w-0">
            <div className="truncate font-mono">{change.change_id}</div>
            {change.title && (
              <div className="truncate text-text-subtle">{change.title}</div>
            )}
          </div>
        </button>
      ))}
    </div>
  );
}
