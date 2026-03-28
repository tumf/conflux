import React, { useEffect, useState } from 'react';
import { Play, Square } from 'lucide-react';
import { ConnectionStatus } from '../api/wsClient';
import { OrchestrationStatus } from '../api/types';
import { fetchVersion } from '../api/restClient';

interface HeaderProps {
  connectionStatus: ConnectionStatus;
  orchestrationStatus: OrchestrationStatus;
  onRun: () => void;
  onStop: () => void;
  isLoading: boolean;
}

const statusConfig: Record<ConnectionStatus, { color: string; label: string }> = {
  connected: { color: 'bg-[#22c55e]', label: 'Connected' },
  reconnecting: { color: 'bg-[#f59e0b] animate-pulse', label: 'Reconnecting' },
  disconnected: { color: 'bg-[#ef4444]', label: 'Disconnected' },
};

export function Header({ connectionStatus, orchestrationStatus, onRun, onStop, isLoading }: HeaderProps) {
  const { color, label } = statusConfig[connectionStatus];
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    fetchVersion()
      .then((data) => setVersion(data.version))
      .catch(() => {
        // Silently ignore fetch failures — version is not displayed
      });
  }, []);

  const isRunning = orchestrationStatus === 'running';

  return (
    <header className="border-b border-[#27272a] bg-[#111113] px-5 py-3">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <div className="size-5 rounded bg-[#6366f1] flex items-center justify-center">
            <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
              <path d="M6 1L11 4V8L6 11L1 8V4L6 1Z" fill="white" fillOpacity="0.9" />
            </svg>
          </div>
          <span className="text-sm font-semibold tracking-tight text-[#fafafa]">Conflux</span>
          {version && (
            <span className="text-xs text-[#52525b]">{version}</span>
          )}
        </div>
        <div className="flex items-center gap-3">
          {/* Global Run/Stop buttons */}
          {isRunning ? (
            <button
              onClick={onStop}
              disabled={isLoading}
              className="flex items-center gap-1.5 rounded-md bg-[#450a0a]/60 px-3 py-1.5 text-xs font-medium text-[#ef4444] transition-colors hover:bg-[#450a0a]/80 disabled:cursor-not-allowed disabled:opacity-40"
              aria-label="Stop orchestration"
            >
              <Square className="size-3" />
              Stop
            </button>
          ) : (
            <button
              onClick={onRun}
              disabled={isLoading}
              className="flex items-center gap-1.5 rounded-md bg-[#166534]/60 px-3 py-1.5 text-xs font-medium text-[#22c55e] transition-colors hover:bg-[#166534]/80 disabled:cursor-not-allowed disabled:opacity-40"
              aria-label="Run orchestration"
            >
              <Play className="size-3" />
              Run
            </button>
          )}

          {/* Connection status */}
          <div className="flex items-center gap-1.5">
            <div className={`size-1.5 rounded-full ${color}`} aria-hidden="true" />
            <span className="text-xs text-[#71717a]">{label}</span>
          </div>
        </div>
      </div>
    </header>
  );
}
