/**
 * ChangeRow Component
 * Displays a single change with progress and status
 */

import React from 'react';
import { RemoteChange } from '../api/types';

interface ChangeRowProps {
  change: RemoteChange;
}

const getStatusColor = (status: string): string => {
  switch (status) {
    case 'idle':
      return 'bg-slate-600';
    case 'queued':
      return 'bg-blue-600';
    case 'applying':
    case 'accepting':
    case 'archiving':
    case 'resolving':
      return 'bg-amber-600';
    case 'archived':
    case 'merged':
      return 'bg-green-600';
    case 'error':
      return 'bg-red-600';
    default:
      return 'bg-gray-600';
  }
};

export function ChangeRow({ change }: ChangeRowProps) {
  const progress =
    change.total_tasks > 0
      ? Math.round((change.completed_tasks / change.total_tasks) * 100)
      : 0;

  const statusDisplay =
    change.iteration_number > 0
      ? `${change.status}:${change.iteration_number}`
      : change.status;

  return (
    <div className="space-y-2 rounded bg-color-surface-secondary p-3">
      <div className="flex items-center justify-between">
        <h4 className="font-semibold text-color-text">{change.id}</h4>
        <span className={`rounded px-2 py-1 text-xs font-bold text-white ${getStatusColor(change.status)}`}>
          {statusDisplay}
        </span>
      </div>

      <div className="space-y-1">
        <div className="flex items-center justify-between text-sm">
          <span className="text-color-text-secondary">Progress</span>
          <span className="text-color-text">
            {change.completed_tasks}/{change.total_tasks}
          </span>
        </div>
        <div className="h-2 w-full rounded bg-color-border">
          <div
            className="h-2 rounded bg-color-accent transition-all"
            style={{ width: `${progress}%` }}
            role="progressbar"
            aria-valuenow={progress}
            aria-valuemin={0}
            aria-valuemax={100}
          />
        </div>
      </div>

      {change.error && (
        <p className="text-sm text-color-error">{change.error}</p>
      )}
    </div>
  );
}
