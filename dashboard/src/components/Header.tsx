/**
 * Header Component
 * Displays application title and connection status
 */

import React from 'react';
import { ConnectionStatus } from '../api/wsClient';

interface HeaderProps {
  connectionStatus: ConnectionStatus;
}

const statusColors: Record<ConnectionStatus, string> = {
  connected: 'bg-green-500',
  reconnecting: 'bg-yellow-500',
  disconnected: 'bg-red-500',
};

const statusText: Record<ConnectionStatus, string> = {
  connected: 'Connected',
  reconnecting: 'Reconnecting...',
  disconnected: 'Disconnected',
};

export function Header({ connectionStatus }: HeaderProps) {
  return (
    <header className="border-b border-color-border bg-color-surface px-6 py-4">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-color-text">Conflux Server</h1>
        <div className="flex items-center gap-2">
          <div
            className={`h-3 w-3 rounded-full ${statusColors[connectionStatus]}`}
            aria-hidden="true"
          />
          <span className="text-sm text-color-text-secondary">
            {statusText[connectionStatus]}
          </span>
        </div>
      </div>
    </header>
  );
}
