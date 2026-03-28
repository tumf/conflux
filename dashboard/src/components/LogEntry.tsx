/**
 * LogEntry Component
 * Displays a single log entry
 */

import React from 'react';
import { RemoteLogEntry } from '../api/types';

interface LogEntryProps {
  entry: Omit<RemoteLogEntry, 'project_id'>;
}

const levelColors: Record<string, string> = {
  info: 'text-color-info',
  warn: 'text-color-warning',
  error: 'text-color-error',
};

const levelBg: Record<string, string> = {
  info: 'bg-blue-900/20',
  warn: 'bg-yellow-900/20',
  error: 'bg-red-900/20',
};

export function LogEntry({ entry }: LogEntryProps) {
  const date = new Date(entry.timestamp);
  const timeStr = date.toLocaleTimeString();

  return (
    <div className={`rounded px-3 py-2 font-mono text-sm ${levelBg[entry.level]}`}>
      <div className="flex items-start gap-2">
        <span className="text-color-text-secondary">{timeStr}</span>
        <span className={`font-bold ${levelColors[entry.level]}`}>
          [{entry.level.toUpperCase()}]
        </span>
        <span className="flex-1 text-color-text">{entry.message}</span>
      </div>
    </div>
  );
}
