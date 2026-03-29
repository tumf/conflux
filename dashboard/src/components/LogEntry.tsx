import React from 'react';
import { RemoteLogEntry } from '../api/types';

interface LogEntryProps {
  entry: RemoteLogEntry;
  showProjectLabel?: boolean;
}

const levelConfig: Record<string, { label: string; color: string; bg: string }> = {
  info: { label: 'INFO', color: 'text-[#3b82f6]', bg: '' },
  warn: { label: 'WARN', color: 'text-[#f59e0b]', bg: 'bg-[#451a03]/20' },
  error: { label: 'ERR ', color: 'text-[#ef4444]', bg: 'bg-[#450a0a]/20' },
};

export function LogEntry({ entry, showProjectLabel = false }: LogEntryProps) {
  const date = new Date(entry.timestamp);
  const timeStr = date.toLocaleTimeString('en', { hour12: false });
  const cfg = levelConfig[entry.level] ?? levelConfig.info;

  return (
    <div className={`flex gap-2 rounded px-2 py-1 font-mono text-xs ${cfg.bg}`}>
      <span className="shrink-0 text-[#3f3f46]">{timeStr}</span>
      <span className={`shrink-0 ${cfg.color}`}>{cfg.label}</span>
      {showProjectLabel && entry.project_id ? (
        <span className="shrink-0 rounded bg-[#18181b] px-1.5 py-0.5 text-[#71717a]">
          {entry.project_id}
        </span>
      ) : null}
      <span className="text-[#a1a1aa] break-all">{entry.message}</span>
    </div>
  );
}
