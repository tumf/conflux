/**
 * WebSocket hook for proposal session real-time communication.
 * Connects to /api/v1/proposal-sessions/{sessionId}/ws
 * and handles all proposal-specific message types.
 */

import { useEffect, useRef, useCallback, useState } from 'react';
import { getProposalSessionWsUrl } from '../api/restClient';
import {
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
  hasActiveTurn?: () => boolean;
  onUserMessage?: (message: { id: string; content: string; timestamp: string }) => void;
  onMessageChunk?: (content: string, messageId?: string, turnId?: string) => void;
  onThoughtChunk?: (content: string, messageId?: string, turnId?: string) => void;
  onToolCall?: (toolCall: ToolCallInfo, messageId?: string, turnId?: string) => void;
  onToolCallUpdate?: (toolCallId: string, status: ToolCallStatus, messageId?: string, turnId?: string) => void;
  onElicitationRequest?: (elicitation: ElicitationRequest) => void;
  onTurnComplete?: (stopReason: string, messageId?: string, turnId?: string) => void;
  onError?: (message: string) => void;
}

export function useProposalWebSocket(options: UseProposalWebSocketOptions) {
  const {
    projectId,
    sessionId,
    onToolCall,
    onToolCallUpdate,
    onElicitationRequest,
    onTurnComplete,
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
      if (callbacksRef.current.hasActiveTurn?.()) {
        callbacksRef.current.onError?.('WebSocket disconnected');
      }
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
    case 'user_message':
      callbacks.onUserMessage?.({
        id: msg.id,
        content: msg.content,
        timestamp: msg.timestamp,
      });
      break;
    case 'agent_message_chunk':
      callbacks.onMessageChunk?.(msg.text, msg.message_id, msg.turn_id);
      break;
    case 'agent_thought_chunk':
      callbacks.onThoughtChunk?.(msg.text, msg.message_id, msg.turn_id);
      break;
    case 'tool_call':
      callbacks.onToolCall?.(
        {
          id: msg.tool_call_id,
          title: msg.title,
          status: msg.status,
        },
        msg.message_id,
        msg.turn_id,
      );
      break;
    case 'tool_call_update':
      callbacks.onToolCallUpdate?.(msg.tool_call_id, msg.status, msg.message_id, msg.turn_id);
      break;
    case 'elicitation':
      callbacks.onElicitationRequest?.({
        id: msg.request_id,
        message: msg.message,
        properties: msg.schema?.properties ?? {},
        required: msg.schema?.required,
      });
      break;
    case 'turn_complete':
      callbacks.onTurnComplete?.(msg.stop_reason, msg.message_id, msg.turn_id);
      break;
    case 'error':
      callbacks.onError?.(msg.message);
      break;
  }
}
