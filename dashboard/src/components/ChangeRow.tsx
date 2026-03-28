import React from 'react';
import { RemoteChange } from '../api/types';

interface ChangeRowProps {
  change: RemoteChange;
}

const statusConfig: Record<string, { color: string; bg: string }> = {
  idle: { color: 'text-[#71717a]', bg: 'bg-[#27272a]' },
  queued: { color: 'text-[#3b82f6]', bg: 'bg-[#1e3a5f]/50' },
  applying: { color: 'text-[#f59e0b]', bg: 'bg-[#451a03]/50' },
  accepting: { color: 'text-[#f59e0b]', bg: 'bg-[#451a03]/50' },
  archiving: { color: 'text-[#f59e0b]', bg: 'bg-[#451a03]/50' },
  resolving: { color: 'text-[#f59e0b]', bg: 'bg-[#451a03]/50' },
  archived: { color: 'text-[#22c55e]', bg: 'bg-[#052e16]/50' },
  merged: { color: 'text-[#22c55e]', bg: 'bg-[#052e16]/50' },
  error: { color: 'text-[#ef4444]', bg: 'bg-[#450a0a]/50' },
};

const progressBarColor: Record<string, string> = {
  idle: 'bg-[#3f3f46]',
  queued: 'bg-[#3b82f6]',
  applying: 'bg-[#f59e0b]',
  accepting: 'bg-[#f59e0b]',
  archiving: 'bg-[#f59e0b]',
  resolving: 'bg-[#f59e0b]',
  archived: 'bg-[#22c55e]',
  merged: 'bg-[#22c55e]',
  error: 'bg-[#ef4444]',
};

export function ChangeRow({ change }: ChangeRowProps) {
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

  return (
    <div className="space-y-2 rounded-md border border-[#27272a] bg-[#111113] p-3">
      <div className="flex items-center justify-between gap-2">
        <span className="truncate font-mono text-xs text-[#a1a1aa]">{change.id}</span>
        <span className={`shrink-0 rounded px-1.5 py-0.5 text-xs font-medium ${cfg.color} ${cfg.bg}`}>
          {statusDisplay}
        </span>
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
