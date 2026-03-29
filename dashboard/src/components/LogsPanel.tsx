import React, { useEffect, useRef } from 'react';
import { RemoteLogEntry } from '../api/types';
import { LogEntry } from './LogEntry';

interface LogsPanelProps {
  logs: RemoteLogEntry[];
  selectedProjectId: string | null;
}

export function LogsPanel({ logs, selectedProjectId }: LogsPanelProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const regionLabel = selectedProjectId ? 'Project logs' : 'Orchestration logs';

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [logs]);

  if (logs.length === 0) {
    return (
      <div className="flex flex-1 items-center justify-center p-8">
        <p className="text-sm text-[#52525b]">No logs yet</p>
      </div>
    );
  }

  return (
    <div
      ref={scrollRef}
      className="space-y-0.5 overflow-y-auto p-3"
      role="region"
      aria-label={regionLabel}
    >
      {logs.map((log, idx) => (
        <LogEntry
          key={`${log.timestamp}-${idx}`}
          entry={log}
          showProjectLabel={!selectedProjectId}
        />
      ))}
    </div>
  );
}
