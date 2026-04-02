import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import {
  ElicitationRequest,
  ProposalChatMessage,
  ProposalWsServerMessage,
  ToolCallInfo,
  ToolCallStatus,
} from '../api/types';
import { getProposalSessionWsUrl, listProposalSessionMessages } from '../api/restClient';

export type ProposalChatStatus = 'ready' | 'submitted' | 'streaming' | 'recovering' | 'error';

interface SubmissionLock {
  isLocked: boolean;
  clearVersion: number;
}

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
  const [submissionLock, setSubmissionLock] = useState<SubmissionLock>({ isLocked: false, clearVersion: 0 });

  const wsRef = useRef<WebSocket | null>(null);
  const pendingPromptsRef = useRef<PendingPrompt[]>([]);
  const reconnectAttemptsRef = useRef(0);
  const reconnectTimerRef = useRef<number | null>(null);
  const unmountedRef = useRef(false);
  const activeAssistantMessageIdRef = useRef<string | null>(null);
  const statusRef = useRef<ProposalChatStatus>('ready');
  const historyLoadedRef = useRef(false);
  const sessionGenerationRef = useRef(0);
  const acceptedPromptIdsRef = useRef<Set<string>>(new Set());
  const recoveryTurnIdRef = useRef<string | null>(null);

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

  const updateSubmissionLock = useCallback((nextLocked: boolean, reason: string) => {
    setSubmissionLock((prev) => {
      if (prev.isLocked !== nextLocked) {
        console.info('proposal-chat submission lock transition', {
          previousLocked: prev.isLocked,
          nextLocked,
          reason,
          at: nowIso(),
        });
      }
      return {
        isLocked: nextLocked,
        clearVersion: nextLocked ? prev.clearVersion : prev.clearVersion + 1,
      };
    });
  }, []);

  const clearSubmissionLock = useCallback(() => {
    setSubmissionLock((prev) => {
      if (!prev.isLocked) return prev;
      return {
        isLocked: false,
        clearVersion: prev.clearVersion + 1,
      };
    });
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
      if (acceptedPromptIdsRef.current.has(prompt.clientMessageId)) {
        console.info('proposal-chat skipped duplicate prompt flush after reconnect', {
          clientMessageId: prompt.clientMessageId,
          at: nowIso(),
        });
        return;
      }

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
      if (current === 'submitted' || current === 'streaming' || current === 'recovering') {
        transitionStatus('error', reason);
        setError(reason);
      }
      clearSubmissionLock();
    },
    [clearSubmissionLock, transitionStatus],
  );

  const enterRecovery = useCallback(
    (reason: string) => {
      const current = statusRef.current;
      if (current === 'submitted' || current === 'streaming') {
        transitionStatus('recovering', reason);
        setError(null);
      }
    },
    [transitionStatus],
  );

  const handleServerMessage = useCallback(
    (msg: ProposalWsServerMessage, generation: number) => {
      if (sessionGenerationRef.current !== generation || unmountedRef.current) {
        console.debug('proposal-chat discard stale websocket event', {
          projectId,
          sessionId,
          generation,
          currentGeneration: sessionGenerationRef.current,
          type: msg.type,
          at: nowIso(),
        });
        return;
      }
      switch (msg.type) {
        case 'user_message': {
          const cid = msg.client_message_id;
          if (cid) {
            acceptedPromptIdsRef.current.add(cid);
            pendingPromptsRef.current = pendingPromptsRef.current.filter(
              (prompt) => prompt.clientMessageId !== cid,
            );
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
            clearSubmissionLock();
            break;
          }

          appendOrUpdateMessage({
            id: msg.id,
            role: 'user',
            content: msg.content,
            timestamp: msg.timestamp,
            sendStatus: 'sent',
          });
          clearSubmissionLock();
          break;
        }
        case 'agent_message_chunk': {
          const messageId = msg.message_id ?? activeAssistantMessageIdRef.current ?? `assistant-${Date.now()}`;
          activeAssistantMessageIdRef.current = messageId;
          if (msg.turn_id) {
            recoveryTurnIdRef.current = msg.turn_id;
          }
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
            const isReplayDuplicate = Boolean(msg.message_id) && target.content === msg.text;
            if (isReplayDuplicate) {
              return prev;
            }
            copy[idx] = {
              ...target,
              content: `${target.content}${msg.text}`,
              turn_id: target.turn_id ?? msg.turn_id,
            };
            return copy;
          });
          transitionStatus('streaming', 'agent_message_chunk');
          setError(null);
          break;
        }
        case 'agent_thought_chunk':
          break;
        case 'tool_call': {
          const messageId = msg.message_id ?? activeAssistantMessageIdRef.current;
          if (!messageId) return;
          if (msg.turn_id) {
            recoveryTurnIdRef.current = msg.turn_id;
          }
          updateToolCall(messageId, {
            id: msg.tool_call_id,
            title: msg.title,
            status: msg.status,
          });
          transitionStatus('streaming', 'tool_call');
          setError(null);
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
          recoveryTurnIdRef.current = null;
          transitionStatus('ready', 'turn_complete');
          setError(null);
          break;
        case 'recovery_state':
          if (msg.active) {
            recoveryTurnIdRef.current = msg.turn_id ?? recoveryTurnIdRef.current;
            transitionStatus('recovering', 'server_recovery_state_active');
            setError(null);
          } else {
            recoveryTurnIdRef.current = null;
            transitionStatus('ready', 'server_recovery_state_ready');
            setError(null);
          }
          break;
        case 'heartbeat':
          // Keepalive signal only; no UI changes needed.
          break;
        case 'error':
          failActiveTurn(msg.message);
          break;
      }
    },
    [
      appendOrUpdateMessage,
      clearSubmissionLock,
      failActiveTurn,
      projectId,
      sessionId,
      transitionStatus,
      updateToolCall,
      updateToolCallStatus,
    ],
  );

  const connect = useCallback(
    (generation: number) => {
      if (!projectId || !sessionId || unmountedRef.current) return;
      if (sessionGenerationRef.current !== generation) {
        console.debug('proposal-chat skip stale websocket connect attempt', {
          projectId,
          sessionId,
          generation,
          currentGeneration: sessionGenerationRef.current,
          at: nowIso(),
        });
        return;
      }

      const ws = new WebSocket(getProposalSessionWsUrl(projectId, sessionId));
      wsRef.current = ws;

      ws.onopen = () => {
        if (sessionGenerationRef.current !== generation || unmountedRef.current) {
          ws.close();
          return;
        }
        console.info('proposal-chat websocket connected', {
          sessionId,
          historyLoaded: historyLoadedRef.current,
          generation,
        });
        setWsConnected(true);
        reconnectAttemptsRef.current = 0;
        clearReconnectTimer();
        flushPendingPrompts();
      };

      ws.onmessage = (event) => {
        if (sessionGenerationRef.current !== generation || unmountedRef.current) {
          console.debug('proposal-chat discard stale websocket frame', {
            projectId,
            sessionId,
            generation,
            currentGeneration: sessionGenerationRef.current,
            at: nowIso(),
          });
          return;
        }
        try {
          handleServerMessage(JSON.parse(event.data) as ProposalWsServerMessage, generation);
        } catch (e) {
          console.error('proposal-chat websocket parse failure', {
            sessionId,
            error: e instanceof Error ? e.message : String(e),
          });
        }
      };

      ws.onerror = () => {
        if (sessionGenerationRef.current !== generation || unmountedRef.current) {
          return;
        }
        console.error('proposal-chat websocket error', { sessionId, generation });
      };

      ws.onclose = () => {
        if (sessionGenerationRef.current !== generation) {
          return;
        }

        setWsConnected(false);
        wsRef.current = null;

        if (unmountedRef.current) return;

        const currentStatus = statusRef.current;
        if (currentStatus === 'submitted' || currentStatus === 'streaming') {
          enterRecovery('WebSocket disconnected during active turn');
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
        reconnectTimerRef.current = window.setTimeout(() => connect(generation), delay);
      };
    },
    [
      clearReconnectTimer,
      enterRecovery,
      failActiveTurn,
      flushPendingPrompts,
      handleServerMessage,
      projectId,
      sessionId,
    ],
  );

  useEffect(() => {
    unmountedRef.current = false;
    const generation = sessionGenerationRef.current + 1;
    sessionGenerationRef.current = generation;

    setMessages([]);
    setError(null);
    setActiveElicitation(null);
    activeAssistantMessageIdRef.current = null;
    pendingPromptsRef.current = [];
    acceptedPromptIdsRef.current = new Set();
    recoveryTurnIdRef.current = null;
    historyLoadedRef.current = false;

    if (!projectId || !sessionId) {
      setWsConnected(false);
      setSubmissionLock({ isLocked: false, clearVersion: 0 });
      transitionStatus('ready', 'missing_session_or_project');
      return;
    }

    console.info('proposal-chat initialize session generation', {
      projectId,
      sessionId,
      generation,
      at: nowIso(),
    });

    void (async () => {
      try {
        const history = await listProposalSessionMessages(projectId, sessionId);
        if (unmountedRef.current || sessionGenerationRef.current !== generation) {
          console.debug('proposal-chat discard stale history response', {
            projectId,
            sessionId,
            generation,
            currentGeneration: sessionGenerationRef.current,
            at: nowIso(),
          });
          return;
        }
        setMessages(history.messages);
      } catch (e) {
        if (sessionGenerationRef.current !== generation) {
          return;
        }
        console.warn('proposal-chat history load failed', {
          sessionId,
          generation,
          error: e instanceof Error ? e.message : String(e),
        });
      } finally {
        if (sessionGenerationRef.current !== generation) {
          return;
        }
        historyLoadedRef.current = true;
        if (!unmountedRef.current) {
          connect(generation);
        }
      }
    })();

    return () => {
      unmountedRef.current = true;
      clearReconnectTimer();
      if (
        wsRef.current &&
        (wsRef.current.readyState === WebSocket.OPEN || wsRef.current.readyState === WebSocket.CONNECTING)
      ) {
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
      if (submissionLock.isLocked) return;

      const clientMessageId = `user-pending-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
      appendOrUpdateMessage({
        id: clientMessageId,
        role: 'user',
        content: trimmed,
        timestamp: nowIso(),
        sendStatus: wsConnected ? 'sent' : 'pending',
      });

      setError(null);
      updateSubmissionLock(true, 'send_message');
      transitionStatus('submitted', 'send_message');

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
    [appendOrUpdateMessage, sessionId, transitionStatus, submissionLock.isLocked, updateSubmissionLock, wsConnected],
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
      submissionLock,
    }),
    [
      activeElicitation,
      error,
      messages,
      sendElicitationResponse,
      sendMessage,
      status,
      stop,
      wsConnected,
      submissionLock,
    ],
  );
}
