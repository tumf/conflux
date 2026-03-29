/**
 * WebSocket hook for proposal session real-time communication.
 * Connects to /api/v1/projects/{projectId}/proposal-sessions/{sessionId}/ws
 * and handles all proposal-specific message types.
 */

import { useEffect, useRef, useCallback, useState } from 'react';
import { getProposalSessionWsUrl } from '../api/restClient';
import {
  ProposalChatMessage,
  ProposalSession,
  ProposalWsClientMessage,
  ProposalWsServerMessage,
  ElicitationRequest,
  ToolCallInfo,
  ToolCallStatus,
} from '../api/types';

export type ProposalWsStatus = 'connecting' | 'connected' | 'disconnected' | 'error';

export interface UseProposalWebSocketOptions {
  projectId: string | null;
  sessionId: string | null;
  onMessage?: (message: ProposalChatMessage) => void;
  onMessageChunk?: (messageId: string, content: string) => void;
  onToolCallStart?: (messageId: string, toolCall: ToolCallInfo) => void;
  onToolCallUpdate?: (messageId: string, toolCallId: string, status: ToolCallStatus) => void;
  onElicitationRequest?: (elicitation: ElicitationRequest) => void;
  onSessionUpdate?: (session: ProposalSession) => void;
  onError?: (message: string) => void;
}

export function useProposalWebSocket(options: UseProposalWebSocketOptions) {
  const {
    projectId,
    sessionId,
    onMessage,
    onMessageChunk,
    onToolCallStart,
    onToolCallUpdate,
    onElicitationRequest,
    onSessionUpdate,
    onError,
  } = options;

  const [status, setStatus] = useState<ProposalWsStatus>('disconnected');
  const wsRef = useRef<WebSocket | null>(null);
  const callbacksRef = useRef(options);
  callbacksRef.current = options;

  // Connect/disconnect based on projectId + sessionId
  useEffect(() => {
    if (!projectId || !sessionId) {
      setStatus('disconnected');
      return;
    }

    const url = getProposalSessionWsUrl(projectId, sessionId);
    setStatus('connecting');

    const ws = new WebSocket(url);
    wsRef.current = ws;

    ws.onopen = () => {
      setStatus('connected');
    };

    ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data) as ProposalWsServerMessage;
        handleServerMessage(msg, callbacksRef.current);
      } catch (err) {
        console.error('Failed to parse proposal WS message:', err);
      }
    };

    ws.onerror = () => {
      setStatus('error');
      callbacksRef.current.onError?.('WebSocket connection error');
    };

    ws.onclose = () => {
      setStatus('disconnected');
      wsRef.current = null;
    };

    return () => {
      ws.onopen = null;
      ws.onmessage = null;
      ws.onerror = null;
      ws.onclose = null;
      if (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING) {
        ws.close();
      }
      wsRef.current = null;
      setStatus('disconnected');
    };
  }, [projectId, sessionId]);

  const sendMessage = useCallback((msg: ProposalWsClientMessage) => {
    const ws = wsRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) {
      console.warn('Proposal WebSocket not connected, cannot send message');
      return;
    }
    ws.send(JSON.stringify(msg));
  }, []);

  const sendPrompt = useCallback(
    (content: string) => {
      sendMessage({ type: 'prompt', content });
    },
    [sendMessage],
  );

  const sendElicitationResponse = useCallback(
    (
      elicitationId: string,
      action: 'accept' | 'decline' | 'cancel',
      data?: Record<string, unknown>,
    ) => {
      sendMessage({
        type: 'elicitation_response',
        elicitation_id: elicitationId,
        action,
        data,
      });
    },
    [sendMessage],
  );

  const sendCancel = useCallback(() => {
    sendMessage({ type: 'cancel' });
  }, [sendMessage]);

  return {
    status,
    sendPrompt,
    sendElicitationResponse,
    sendCancel,
  };
}

export function handleServerMessage(
  msg: ProposalWsServerMessage,
  callbacks: UseProposalWebSocketOptions,
) {
  switch (msg.type) {
    case 'assistant_message':
      callbacks.onMessage?.(msg.message);
      break;
    case 'assistant_chunk':
      callbacks.onMessageChunk?.(msg.message_id, msg.content);
      break;
    case 'tool_call_start':
      callbacks.onToolCallStart?.(msg.message_id, msg.tool_call);
      break;
    case 'tool_call_update':
      callbacks.onToolCallUpdate?.(msg.message_id, msg.tool_call_id, msg.status);
      break;
    case 'elicitation_request':
      callbacks.onElicitationRequest?.(msg.elicitation);
      break;
    case 'session_update':
      callbacks.onSessionUpdate?.(msg.session);
      break;
    case 'error':
      callbacks.onError?.(msg.message);
      break;
  }
}
