/**
 * LogsPanel Component
 * Displays logs for the selected project with auto-scroll
 */

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
    // Auto-scroll to bottom when new logs arrive
    if (scrollRef.current) {
      const { scrollHeight, clientHeight } = scrollRef.current;
      scrollRef.current.scrollTop = scrollHeight - clientHeight;
    }
  }, [logs]);

  if (!selectedProjectId) {
    return (
      <div className="flex flex-1 items-center justify-center p-8">
        <p className="text-color-text-secondary">Select a project to view logs</p>
      </div>
    );
  }

  if (logs.length === 0) {
    return (
      <div className="flex flex-1 items-center justify-center p-8">
        <p className="text-color-text-secondary">No logs yet</p>
      </div>
    );
  }

  return (
    <div
      ref={scrollRef}
      className="flex-1 space-y-1 overflow-y-auto p-4"
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
