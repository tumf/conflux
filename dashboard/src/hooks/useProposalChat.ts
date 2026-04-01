import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import {
  ElicitationRequest,
  ProposalChatMessage,
  ProposalWsServerMessage,
  ToolCallInfo,
  ToolCallStatus,
} from '../api/types';
import { getProposalSessionWsUrl, listProposalSessionMessages } from '../api/restClient';

export type ProposalChatStatus = 'ready' | 'submitted' | 'streaming' | 'error';

interface PendingPrompt {
  content: string;
  clientMessageId: string;
}

const RECONNECT_DELAYS_MS = [1000, 2000, 4000, 8000, 16000];
const MAX_RECONNECT_DELAY_MS = 30000;
const MAX_RETRIES = 10;

function nowIso(): string {
  return new Date().toISOString();
}

function toElicitation(msg: Extract<ProposalWsServerMessage, { type: 'elicitation' }>): ElicitationRequest {
  return {
    id: msg.request_id,
    message: msg.message,
    properties: msg.schema?.properties ?? {},
    required: msg.schema?.required,
  };
}

export function useProposalChat(projectId: string | null, sessionId: string | null) {
  const [messages, setMessages] = useState<ProposalChatMessage[]>([]);
  const [status, setStatus] = useState<ProposalChatStatus>('ready');
  const [error, setError] = useState<string | null>(null);
  const [activeElicitation, setActiveElicitation] = useState<ElicitationRequest | null>(null);
  const [wsConnected, setWsConnected] = useState(false);

  const wsRef = useRef<WebSocket | null>(null);
  const pendingPromptsRef = useRef<PendingPrompt[]>([]);
  const reconnectAttemptsRef = useRef(0);
  const reconnectTimerRef = useRef<number | null>(null);
  const unmountedRef = useRef(false);
  const activeAssistantMessageIdRef = useRef<string | null>(null);
  const statusRef = useRef<ProposalChatStatus>('ready');

  const transitionStatus = useCallback((next: ProposalChatStatus, reason: string) => {
    setStatus((prev) => {
      if (prev !== next) {
        console.info('proposal-chat status transition', { prev, next, reason, at: nowIso() });
      }
      statusRef.current = next;
      return next;
    });
  }, []);

  const clearReconnectTimer = useCallback(() => {
    if (reconnectTimerRef.current !== null) {
      window.clearTimeout(reconnectTimerRef.current);
      reconnectTimerRef.current = null;
    }
  }, []);

  const appendOrUpdateMessage = useCallback((nextMessage: ProposalChatMessage) => {
    setMessages((prev) => {
      const idx = prev.findIndex((m) => m.id === nextMessage.id);
      if (idx === -1) return [...prev, nextMessage];
      const copy = [...prev];
      copy[idx] = { ...copy[idx], ...nextMessage };
      return copy;
    });
  }, []);

  const updateToolCall = useCallback((messageId: string, toolCall: ToolCallInfo) => {
    setMessages((prev) =>
      prev.map((m) => {
        if (m.id !== messageId) return m;
        const current = m.tool_calls ?? [];
        const idx = current.findIndex((tc) => tc.id === toolCall.id);
        const nextToolCalls =
          idx === -1
            ? [...current, toolCall]
            : current.map((tc, i) => (i === idx ? toolCall : tc));
        return { ...m, tool_calls: nextToolCalls };
      }),
    );
  }, []);

  const updateToolCallStatus = useCallback((messageId: string, toolCallId: string, nextStatus: ToolCallStatus) => {
    setMessages((prev) =>
      prev.map((m) => {
        if (m.id !== messageId || !m.tool_calls) return m;
        return {
          ...m,
          tool_calls: m.tool_calls.map((tc) =>
            tc.id === toolCallId ? { ...tc, status: nextStatus } : tc,
          ),
        };
      }),
    );
  }, []);

  const flushPendingPrompts = useCallback(() => {
    const ws = wsRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) return;
    if (pendingPromptsRef.current.length === 0) return;

    const queue = [...pendingPromptsRef.current];
    pendingPromptsRef.current = [];
    queue.forEach((prompt) => {
      ws.send(
        JSON.stringify({
          type: 'prompt',
          content: prompt.content,
          client_message_id: prompt.clientMessageId,
        }),
      );
    });
  }, []);

  const failActiveTurn = useCallback(
    (reason: string) => {
      const current = statusRef.current;
      if (current === 'submitted' || current === 'streaming') {
        transitionStatus('error', reason);
        setError(reason);
      }
    },
    [transitionStatus],
  );

  const handleServerMessage = useCallback(
    (msg: ProposalWsServerMessage) => {
      switch (msg.type) {
        case 'user_message': {
          const cid = msg.client_message_id;
          if (cid) {
            setMessages((prev) =>
              prev.map((m) =>
                m.id === cid
                  ? {
                      ...m,
                      id: msg.id,
                      content: msg.content,
                      timestamp: msg.timestamp,
                      sendStatus: 'sent',
                    }
                  : m,
              ),
            );
          } else {
            appendOrUpdateMessage({
              id: msg.id,
              role: 'user',
              content: msg.content,
              timestamp: msg.timestamp,
              sendStatus: 'sent',
            });
          }
          break;
        }
        case 'agent_message_chunk': {
          const messageId = msg.message_id ?? activeAssistantMessageIdRef.current ?? `assistant-${Date.now()}`;
          activeAssistantMessageIdRef.current = messageId;
          setMessages((prev) => {
            const idx = prev.findIndex((m) => m.id === messageId);
            if (idx === -1) {
              return [
                ...prev,
                {
                  id: messageId,
                  role: 'assistant',
                  content: msg.text,
                  timestamp: nowIso(),
                  turn_id: msg.turn_id,
                },
              ];
            }
            const copy = [...prev];
            const target = copy[idx];
            copy[idx] = {
              ...target,
              content: `${target.content}${msg.text}`,
              turn_id: target.turn_id ?? msg.turn_id,
            };
            return copy;
          });
          transitionStatus('streaming', 'agent_message_chunk');
          break;
        }
        case 'agent_thought_chunk':
          break;
        case 'tool_call': {
          const messageId = msg.message_id ?? activeAssistantMessageIdRef.current;
          if (!messageId) return;
          updateToolCall(messageId, {
            id: msg.tool_call_id,
            title: msg.title,
            status: msg.status,
          });
          break;
        }
        case 'tool_call_update': {
          const messageId = msg.message_id ?? activeAssistantMessageIdRef.current;
          if (!messageId) return;
          updateToolCallStatus(messageId, msg.tool_call_id, msg.status);
          break;
        }
        case 'elicitation':
          setActiveElicitation(toElicitation(msg));
          break;
        case 'turn_complete':
          activeAssistantMessageIdRef.current = null;
          transitionStatus('ready', 'turn_complete');
          setError(null);
          break;
        case 'error':
          failActiveTurn(msg.message);
          break;
      }
    },
    [appendOrUpdateMessage, failActiveTurn, transitionStatus, updateToolCall, updateToolCallStatus],
  );

  const connect = useCallback(() => {
    if (!projectId || !sessionId || unmountedRef.current) return;

    const ws = new WebSocket(getProposalSessionWsUrl(projectId, sessionId));
    wsRef.current = ws;

    ws.onopen = () => {
      console.info('proposal-chat websocket connected', { sessionId });
      setWsConnected(true);
      reconnectAttemptsRef.current = 0;
      clearReconnectTimer();
      flushPendingPrompts();
    };

    ws.onmessage = (event) => {
      try {
        handleServerMessage(JSON.parse(event.data) as ProposalWsServerMessage);
      } catch (e) {
        console.error('proposal-chat websocket parse failure', {
          sessionId,
          error: e instanceof Error ? e.message : String(e),
        });
      }
    };

    ws.onerror = () => {
      console.error('proposal-chat websocket error', { sessionId });
    };

    ws.onclose = () => {
      setWsConnected(false);
      wsRef.current = null;

      if (unmountedRef.current) return;

      const currentStatus = statusRef.current;
      if (currentStatus === 'submitted' || currentStatus === 'streaming') {
        failActiveTurn('WebSocket disconnected during active turn');
      }

      if (reconnectAttemptsRef.current >= MAX_RETRIES) {
        failActiveTurn('WebSocket reconnect limit reached');
        return;
      }

      const nextAttempt = reconnectAttemptsRef.current + 1;
      reconnectAttemptsRef.current = nextAttempt;
      const delay = Math.min(
        RECONNECT_DELAYS_MS[Math.min(nextAttempt - 1, RECONNECT_DELAYS_MS.length - 1)],
        MAX_RECONNECT_DELAY_MS,
      );
      reconnectTimerRef.current = window.setTimeout(connect, delay);
    };
  }, [clearReconnectTimer, failActiveTurn, flushPendingPrompts, handleServerMessage, projectId, sessionId]);

  useEffect(() => {
    unmountedRef.current = false;
    setMessages([]);
    setError(null);
    setActiveElicitation(null);
    activeAssistantMessageIdRef.current = null;
    pendingPromptsRef.current = [];

    if (!projectId || !sessionId) {
      setWsConnected(false);
      transitionStatus('ready', 'missing_session_or_project');
      return;
    }

    listProposalSessionMessages(projectId, sessionId)
      .then((history) => {
        if (unmountedRef.current) return;
        setMessages(history.messages);
      })
      .catch((e) => {
        console.warn('proposal-chat history load failed', {
          sessionId,
          error: e instanceof Error ? e.message : String(e),
        });
      });

    connect();

    return () => {
      unmountedRef.current = true;
      clearReconnectTimer();
      if (wsRef.current && (wsRef.current.readyState === WebSocket.OPEN || wsRef.current.readyState === WebSocket.CONNECTING)) {
        wsRef.current.close();
      }
      wsRef.current = null;
      setWsConnected(false);
    };
  }, [clearReconnectTimer, connect, projectId, sessionId, transitionStatus]);

  const sendMessage = useCallback(
    (content: string) => {
      if (!sessionId) return;
      const trimmed = content.trim();
      if (!trimmed) return;
      if (statusRef.current !== 'ready') return;

      const clientMessageId = `user-pending-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
      appendOrUpdateMessage({
        id: clientMessageId,
        role: 'user',
        content: trimmed,
        timestamp: nowIso(),
        sendStatus: wsConnected ? 'sent' : 'pending',
      });

      transitionStatus('submitted', 'send_message');
      setError(null);

      const payload = JSON.stringify({
        type: 'prompt',
        content: trimmed,
        client_message_id: clientMessageId,
      });

      const ws = wsRef.current;
      if (!ws || ws.readyState !== WebSocket.OPEN) {
        pendingPromptsRef.current.push({ content: trimmed, clientMessageId });
        return;
      }

      ws.send(payload);
    },
    [appendOrUpdateMessage, sessionId, transitionStatus, wsConnected],
  );

  const stop = useCallback(() => {
    const ws = wsRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) return;
    ws.send(JSON.stringify({ type: 'cancel' }));
  }, []);

  const sendElicitationResponse = useCallback(
    (elicitationId: string, action: 'accept' | 'decline' | 'cancel', data?: Record<string, unknown>) => {
      const ws = wsRef.current;
      if (!ws || ws.readyState !== WebSocket.OPEN) {
        setError('WebSocket not connected');
        transitionStatus('error', 'elicitation_without_connection');
        return;
      }
      ws.send(
        JSON.stringify({
          type: 'elicitation_response',
          elicitation_id: elicitationId,
          action,
          data,
        }),
      );
      setActiveElicitation(null);
    },
    [transitionStatus],
  );

  return useMemo(
    () => ({
      messages,
      status,
      sendMessage,
      stop,
      error,
      activeElicitation,
      sendElicitationResponse,
      wsConnected,
    }),
    [activeElicitation, error, messages, sendElicitationResponse, sendMessage, status, stop, wsConnected],
  );
}
