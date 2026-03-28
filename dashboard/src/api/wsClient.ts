/**
 * WebSocket Client for Real-time State Updates
 */

import { FullState, RemoteChange, RemoteProject } from './types';

export type ConnectionStatus = 'connected' | 'reconnecting' | 'disconnected';

interface WSMessage {
  type: 'full_state' | 'ping' | 'pong';
  /** Server sends projects with nested changes */
  projects?: RemoteProject[];
  /** Per-project worktree information */
  worktrees?: FullState['worktrees'];
}

export class WebSocketClient {
  private ws: WebSocket | null = null;
  private url: string;
  private listeners: {
    onStateUpdate?: (state: FullState) => void;
    onConnectionChange?: (status: ConnectionStatus) => void;
    onError?: (error: Error) => void;
  } = {};

  private reconnectAttempts = 0;
  private maxReconnectAttempts = 10;
  private reconnectDelays = [1000, 2000, 4000, 8000, 16000]; // ms, then max 30s
  private maxReconnectDelay = 30000;
  private reconnectTimeoutId: number | null = null;
  private pingTimeoutId: number | null = null;

  constructor() {
    const protocol = window.location.protocol === 'https:' ? 'wss' : 'ws';
    this.url = `${protocol}://${window.location.host}/api/v1/ws`;
  }

  /**
   * Connect to the WebSocket server
   */
  connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      try {
        this.ws = new WebSocket(this.url);

        this.ws.onopen = () => {
          this.reconnectAttempts = 0;
          this.notifyConnectionChange('connected');
          this.startPingTimer();
          resolve();
        };

        this.ws.onmessage = (event) => {
          try {
            const message = JSON.parse(event.data);
            console.debug('WS message received:', message.type);

            if (message.type === 'full_state') {
              const projects: RemoteProject[] = message.projects ?? [];
              // Flatten changes from nested project structure
              const changes: RemoteChange[] = projects.flatMap(
                (project) => project.changes ?? [],
              );
              const state: FullState = {
                projects,
                changes,
                worktrees: message.worktrees,
              };
              this.listeners.onStateUpdate?.(state);
            }
          } catch (err) {
            console.error('Failed to parse WS message:', err, 'raw:', event.data?.substring?.(0, 100));
          }
        };

        this.ws.onerror = (event) => {
          if (this.ws?.readyState === WebSocket.CLOSED || this.ws?.readyState === WebSocket.CLOSING) return;
          const error = new Error(`WebSocket error: ${event}`);
          this.listeners.onError?.(error);
          console.error('WebSocket error:', event);
        };

        this.ws.onclose = () => {
          this.notifyConnectionChange('disconnected');
          this.attemptReconnect();
        };
      } catch (err) {
        reject(err);
      }
    });
  }

  /**
   * Disconnect from the WebSocket server
   */
  disconnect(): void {
    if (this.reconnectTimeoutId !== null) {
      clearTimeout(this.reconnectTimeoutId);
      this.reconnectTimeoutId = null;
    }
    if (this.pingTimeoutId !== null) {
      clearTimeout(this.pingTimeoutId);
      this.pingTimeoutId = null;
    }
    if (this.ws) {
      this.ws.onerror = null;
      this.ws.onclose = null;
      this.ws.onmessage = null;
      this.ws.close();
      this.ws = null;
    }
    this.notifyConnectionChange('disconnected');
  }

  /**
   * Register a listener
   */
  on(
    event: 'stateUpdate' | 'connectionChange' | 'error',
    callback: (data: any) => void,
  ): void {
    if (event === 'stateUpdate') {
      this.listeners.onStateUpdate = callback;
    } else if (event === 'connectionChange') {
      this.listeners.onConnectionChange = callback;
    } else if (event === 'error') {
      this.listeners.onError = callback;
    }
  }

  /**
   * Check if connected
   */
  isConnected(): boolean {
    return this.ws?.readyState === WebSocket.OPEN;
  }

  /**
   * Private: Attempt to reconnect with exponential backoff
   */
  private attemptReconnect(): void {
    if (this.reconnectAttempts >= this.maxReconnectAttempts) {
      console.error('Max reconnection attempts reached');
      return;
    }

    this.reconnectAttempts++;
    this.notifyConnectionChange('reconnecting');

    const delayIndex = Math.min(
      this.reconnectAttempts - 1,
      this.reconnectDelays.length - 1,
    );
    const delay = Math.min(this.reconnectDelays[delayIndex], this.maxReconnectDelay);

    this.reconnectTimeoutId = window.setTimeout(() => {
      this.connect().catch((err) => {
        console.error('Reconnect failed:', err);
        this.attemptReconnect();
      });
    }, delay);
  }

  /**
   * Private: Notify connection status change
   */
  private notifyConnectionChange(status: ConnectionStatus): void {
    this.listeners.onConnectionChange?.(status);
  }

  /**
   * Private: Start periodic ping to keep connection alive
   */
  private startPingTimer(): void {
    this.pingTimeoutId = window.setTimeout(() => {
      if (this.isConnected()) {
        try {
          this.ws!.send(JSON.stringify({ type: 'ping' }));
          this.startPingTimer();
        } catch (err) {
          console.error('Failed to send ping:', err);
        }
      }
    }, 30000); // Every 30 seconds
  }
}

// Singleton instance
export const wsClient = new WebSocketClient();
