/**
 * WebSocket Hook: Connect ws client to application store
 */

import { useEffect, useRef } from 'react';
import { wsClient } from '../api/wsClient';
import { FullState, RemoteLogEntry } from '../api/types';

export interface UseWebSocketOptions {
  onStateUpdate?: (state: FullState) => void;
  onLogEntry?: (entry: RemoteLogEntry) => void;
  onConnectionChange?: (status: 'connected' | 'reconnecting' | 'disconnected') => void;
  onError?: (error: Error) => void;
}

export function useWebSocket(options: UseWebSocketOptions = {}) {
  const { onStateUpdate, onLogEntry, onConnectionChange, onError } = options;
  const callbacksRef = useRef(options);
  callbacksRef.current = options;

  useEffect(() => {
    wsClient.on('stateUpdate', (state: FullState) => {
      callbacksRef.current.onStateUpdate?.(state);
    });
    wsClient.on('logEntry', (entry: RemoteLogEntry) => {
      callbacksRef.current.onLogEntry?.(entry);
    });
    wsClient.on('connectionChange', (status: 'connected' | 'reconnecting' | 'disconnected') => {
      callbacksRef.current.onConnectionChange?.(status);
    });
    wsClient.on('error', (error: Error) => {
      callbacksRef.current.onError?.(error);
    });

    wsClient.connect().catch((err) => {
      console.error('Failed to connect WebSocket:', err);
      callbacksRef.current.onError?.(err);
    });

    return () => {
      wsClient.disconnect();
    };
  }, []);

  return {
    isConnected: () => wsClient.isConnected(),
    disconnect: () => wsClient.disconnect(),
  };
}
