import React from 'react';
import { ConnectionStatus } from '../api/wsClient';

interface HeaderProps {
  connectionStatus: ConnectionStatus;
}

const statusConfig: Record<ConnectionStatus, { color: string; label: string }> = {
  connected: { color: 'bg-[#22c55e]', label: 'Connected' },
  reconnecting: { color: 'bg-[#f59e0b] animate-pulse', label: 'Reconnecting' },
  disconnected: { color: 'bg-[#ef4444]', label: 'Disconnected' },
};

export function Header({ connectionStatus }: HeaderProps) {
  const { color, label } = statusConfig[connectionStatus];

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
        </div>
        <div className="flex items-center gap-1.5">
          <div className={`size-1.5 rounded-full ${color}`} aria-hidden="true" />
          <span className="text-xs text-[#71717a]">{label}</span>
        </div>
      </div>
    </header>
  );
}
