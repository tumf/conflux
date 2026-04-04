import React, { useCallback } from 'react';
import { RemoteChange } from '../api/types';
import { stopAndDequeueChange, toggleChangeSelection } from '../api/restClient';

interface ChangeRowProps {
  change: RemoteChange;
  onClickChange?: (changeId: string) => void;
  isSelected?: boolean;
}

const statusConfig: Record<string, { color: string; bg: string }> = {
  idle: { color: 'text-[#71717a]', bg: 'bg-[#27272a]' },
  'not queued': { color: 'text-[#71717a]', bg: 'bg-[#27272a]' },
  queued: { color: 'text-[#3b82f6]', bg: 'bg-[#1e3a5f]/50' },
  applying: { color: 'text-[#f59e0b]', bg: 'bg-[#451a03]/50' },
  accepting: { color: 'text-[#f59e0b]', bg: 'bg-[#451a03]/50' },
  archiving: { color: 'text-[#f59e0b]', bg: 'bg-[#451a03]/50' },
  resolving: { color: 'text-[#f59e0b]', bg: 'bg-[#451a03]/50' },
  archived: { color: 'text-[#22c55e]', bg: 'bg-[#052e16]/50' },
  merged: { color: 'text-[#22c55e]', bg: 'bg-[#052e16]/50' },
  rejected: { color: 'text-[#f87171]', bg: 'bg-[#7f1d1d]/50' },
  error: { color: 'text-[#ef4444]', bg: 'bg-[#450a0a]/50' },
};

const progressBarColor: Record<string, string> = {
  idle: 'bg-[#3f3f46]',
  'not queued': 'bg-[#3f3f46]',
  queued: 'bg-[#3b82f6]',
  applying: 'bg-[#f59e0b]',
  accepting: 'bg-[#f59e0b]',
  archiving: 'bg-[#f59e0b]',
  resolving: 'bg-[#f59e0b]',
  archived: 'bg-[#22c55e]',
  merged: 'bg-[#22c55e]',
  rejected: 'bg-[#f87171]',
  error: 'bg-[#ef4444]',
};

export function ChangeRow({ change, onClickChange, isSelected }: ChangeRowProps) {
  const progress =
    change.total_tasks > 0
      ? Math.round((change.completed_tasks / change.total_tasks) * 100)
      : 0;

  const statusDisplay =
    change.iteration_number != null && change.iteration_number > 0
      ? `${change.status}:${change.iteration_number}`
      : change.status;

  const cfg = statusConfig[change.status] ?? statusConfig.idle;
  const barColor = progressBarColor[change.status] ?? progressBarColor.idle;

  const handleToggle = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      toggleChangeSelection(change.project, change.id).catch(console.error);
    },
    [change.project, change.id],
  );

  const isActive = ['applying', 'accepting', 'archiving', 'resolving'].includes(change.status);

  const handleStopAndDequeue = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      stopAndDequeueChange(change.project, change.id).catch(console.error);
    },
    [change.project, change.id],
  );

  const handleRowClick = useCallback(() => {
    onClickChange?.(change.id);
  }, [change.id, onClickChange]);

  return (
    <div
      onClick={handleRowClick}
      className={`space-y-2 rounded-md border p-3 cursor-pointer transition-colors ${
        isSelected
          ? 'border-[#6366f1] bg-[#1e1b4b]/30'
          : change.selected
            ? 'border-[#27272a] bg-[#111113] hover:border-[#3f3f46]'
            : 'border-[#27272a]/50 bg-[#111113]/50 opacity-60 hover:border-[#3f3f46]'
      }`}
    >
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2 min-w-0">
          <button
            type="button"
            role="checkbox"
            aria-checked={change.selected}
            aria-label={`Select change ${change.id}`}
            onClick={handleToggle}
            className={`flex h-4 w-4 shrink-0 items-center justify-center rounded border transition-colors ${
              change.selected
                ? 'border-[#3b82f6] bg-[#3b82f6] text-white'
                : 'border-[#52525b] bg-transparent text-transparent hover:border-[#71717a]'
            }`}
          >
            {change.selected && (
              <svg className="h-3 w-3" viewBox="0 0 12 12" fill="none">
                <path
                  d="M2.5 6L5 8.5L9.5 3.5"
                  stroke="currentColor"
                  strokeWidth="1.5"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              </svg>
            )}
          </button>
          <span className="truncate font-mono text-xs text-[#a1a1aa]">{change.id}</span>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          {isActive && (
            <button
              type="button"
              onClick={handleStopAndDequeue}
              className="rounded border border-[#dc2626] px-2 py-0.5 text-xs font-medium text-[#fca5a5] transition-colors hover:bg-[#7f1d1d]/40"
              aria-label={`Stop and dequeue ${change.id}`}
            >
              Stop & dequeue
            </button>
          )}
          <span className={`rounded px-1.5 py-0.5 text-xs font-medium ${cfg.color} ${cfg.bg}`}>
            {statusDisplay}
          </span>
        </div>
      </div>

      <div className="space-y-1">
        <div className="flex justify-between text-xs text-[#52525b]">
          <span>{change.completed_tasks}/{change.total_tasks} tasks</span>
          <span>{progress}%</span>
        </div>
        <div className="h-1 w-full overflow-hidden rounded-full bg-[#27272a]">
          <div
            className={`h-1 rounded-full transition-all duration-300 ${barColor}`}
            style={{ width: `${progress}%` }}
            role="progressbar"
            aria-valuenow={progress}
            aria-valuemin={0}
            aria-valuemax={100}
          />
        </div>
      </div>

      {change.status === 'error' && (
        <p className="text-xs text-[#ef4444]">Error</p>
      )}
    </div>
  );
}
