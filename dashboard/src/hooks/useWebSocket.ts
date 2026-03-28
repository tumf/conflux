/**
 * WebSocket Hook: Connect ws client to application store
 */

import { useEffect } from 'react';
import { wsClient } from '../api/wsClient';
import { FullState } from '../api/types';

export interface UseWebSocketOptions {
  onStateUpdate?: (state: FullState) => void;
  onConnectionChange?: (status: 'connected' | 'reconnecting' | 'disconnected') => void;
  onError?: (error: Error) => void;
}

export function useWebSocket(options: UseWebSocketOptions = {}) {
  const { onStateUpdate, onConnectionChange, onError } = options;

  useEffect(() => {
    // Register listeners
    if (onStateUpdate) {
      wsClient.on('stateUpdate', onStateUpdate);
    }
    if (onConnectionChange) {
      wsClient.on('connectionChange', onConnectionChange);
    }
    if (onError) {
      wsClient.on('error', onError);
    }

    // Connect
    wsClient.connect().catch((err) => {
      console.error('Failed to connect WebSocket:', err);
      onError?.(err);
    });

    // Cleanup on unmount
    return () => {
      wsClient.disconnect();
    };
  }, [onStateUpdate, onConnectionChange, onError]);

  return {
    isConnected: () => wsClient.isConnected(),
    disconnect: () => wsClient.disconnect(),
  };
}
