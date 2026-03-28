import React, { useEffect, useRef } from 'react';
import { RemoteLogEntry } from '../api/types';
import { LogEntry } from './LogEntry';

interface LogsPanelProps {
  logs: RemoteLogEntry[];
  selectedProjectId: string | null;
}

export function LogsPanel({ logs, selectedProjectId }: LogsPanelProps) {
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [logs]);

  if (!selectedProjectId) {
    return (
      <div className="flex flex-1 items-center justify-center p-8">
        <p className="text-sm text-[#52525b]">Select a project to view logs</p>
      </div>
    );
  }

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
      aria-label="Project logs"
    >
      {logs.map((log, idx) => (
        <LogEntry
          key={`${log.timestamp}-${idx}`}
          entry={{
            timestamp: log.timestamp,
            level: log.level,
            message: log.message,
          }}
        />
      ))}
    </div>
  );
}
