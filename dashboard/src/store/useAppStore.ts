/**
 * Application State Management using useReducer
 */

import { useReducer, useCallback } from 'react';
import {
  RemoteProject,
  RemoteLogEntry,
  FullState,
  WorktreeInfo,
  OrchestrationStatus,
  FileBrowseContext,
  ProposalSession,
  ProposalChatMessage,
  ElicitationRequest,
  ToolCallInfo,
  ToolCallStatus,
  ActiveCommand,
} from '../api/types';
import { ConnectionStatus } from '../api/wsClient';

export interface ActiveAssistantTurn {
  messageId: string;
  turnId?: string;
  content: string;
  toolCalls: ToolCallInfo[];
  startedAt: string;
}

export interface AppState {
  projects: RemoteProject[];
  selectedProjectId: string | null;
  logsByProjectId: Record<string, RemoteLogEntry[]>;
  connectionStatus: ConnectionStatus;
  worktreesByProjectId: Record<string, WorktreeInfo[]>;
  /** Whether git/sync is available (resolve_command is configured on server) */
  syncAvailable: boolean;
  /** Global orchestration status */
  orchestrationStatus: OrchestrationStatus;
  /** File browser context (change or worktree selection) */
  fileBrowseContext: FileBrowseContext | null;
  /** Proposal sessions indexed by project ID */
  proposalSessionsByProjectId: Record<string, ProposalSession[]>;
  /** Currently active proposal session ID */
  activeProposalSessionId: string | null;
  /** Chat messages indexed by session ID */
  chatMessagesBySessionId: Record<string, ProposalChatMessage[]>;
  /** Per-session assistant turn that is currently streaming */
  activeTurnBySessionId: Record<string, ActiveAssistantTurn | undefined>;
  /** Active elicitation request (only one at a time) */
  activeElicitation: ElicitationRequest | null;
  /** Whether the agent is currently responding */
  isAgentResponding: boolean;
  /** Streaming message content being built (keyed by message_id) */
  streamingContent: Record<string, string>;
  /** Currently active commands across all worktree roots */
  activeCommands: ActiveCommand[];
}

export type AppAction =
  | { type: 'SET_FULL_STATE'; payload: FullState }
  | { type: 'APPEND_LOG'; payload: RemoteLogEntry }
  | { type: 'SET_CONNECTION_STATUS'; payload: ConnectionStatus }
  | { type: 'SELECT_PROJECT'; payload: string | null }
  | { type: 'CLEAR_LOGS'; payload: string }
  | { type: 'SET_WORKTREES'; payload: { projectId: string; worktrees: WorktreeInfo[] } }
  | { type: 'SET_FILE_BROWSE_CONTEXT'; payload: FileBrowseContext | null }
  | { type: 'SET_PROPOSAL_SESSIONS'; payload: { projectId: string; sessions: ProposalSession[] } }
  | { type: 'ADD_PROPOSAL_SESSION'; payload: { projectId: string; session: ProposalSession } }
  | { type: 'UPDATE_PROPOSAL_SESSION'; payload: ProposalSession }
  | { type: 'REMOVE_PROPOSAL_SESSION'; payload: { projectId: string; sessionId: string } }
  | { type: 'SET_ACTIVE_PROPOSAL_SESSION'; payload: string | null }
  | { type: 'HYDRATE_CHAT_MESSAGES'; payload: { sessionId: string; messages: ProposalChatMessage[] } }
  | { type: 'APPEND_CHAT_MESSAGE'; payload: { sessionId: string; message: ProposalChatMessage } }
  | { type: 'START_ASSISTANT_TURN'; payload: { sessionId: string; messageId: string; turnId?: string } }
  | { type: 'APPEND_STREAMING_CHUNK'; payload: { sessionId: string; messageId: string; content: string; turnId?: string } }
  | { type: 'COMPLETE_ASSISTANT_TURN'; payload: { sessionId: string; stopReason?: string } }
  | { type: 'FAIL_ASSISTANT_TURN'; payload: { sessionId: string; error: string } }
  | { type: 'UPDATE_TOOL_CALL'; payload: { sessionId: string; messageId: string; turnId?: string; toolCall: ToolCallInfo } }
  | { type: 'UPDATE_TOOL_CALL_STATUS'; payload: { sessionId: string; messageId: string; turnId?: string; toolCallId: string; status: ToolCallStatus } }
  | { type: 'SET_ELICITATION'; payload: ElicitationRequest | null }
  | { type: 'SET_AGENT_RESPONDING'; payload: boolean };

const initialState: AppState = {
  projects: [],
  selectedProjectId: null,
  logsByProjectId: {},
  connectionStatus: 'disconnected',
  worktreesByProjectId: {},
  syncAvailable: false,
  orchestrationStatus: 'idle',
  fileBrowseContext: null,
  proposalSessionsByProjectId: {},
  activeProposalSessionId: null,
  chatMessagesBySessionId: {},
  activeTurnBySessionId: {},
  activeElicitation: null,
  isAgentResponding: false,
  streamingContent: {},
  activeCommands: [],
};

export function appReducer(state: AppState, action: AppAction): AppState {
  switch (action.type) {
    case 'SET_FULL_STATE': {
      const newState: AppState = {
        ...state,
        projects: action.payload.projects,
        syncAvailable: action.payload.sync_available ?? false,
        orchestrationStatus: action.payload.orchestration_status ?? 'idle',
        activeCommands: action.payload.active_commands ?? [],
      };
      // Update worktrees if included in full_state
      if (action.payload.worktrees) {
        newState.worktreesByProjectId = {
          ...state.worktreesByProjectId,
          ...action.payload.worktrees,
        };
      }
      return newState;
    }

    case 'SET_WORKTREES': {
      return {
        ...state,
        worktreesByProjectId: {
          ...state.worktreesByProjectId,
          [action.payload.projectId]: action.payload.worktrees,
        },
      };
    }

    case 'APPEND_LOG': {
      const logEntry = action.payload;
      const projectId = logEntry.project_id;
      if (!projectId) {
        // Log entries without a project_id are not stored per-project
        return state;
      }
      const logs = state.logsByProjectId[projectId] || [];
      const newLogs = [...logs, logEntry];
      // Keep only last 500 logs per project
      const trimmedLogs = newLogs.slice(-500);

      return {
        ...state,
        logsByProjectId: {
          ...state.logsByProjectId,
          [projectId]: trimmedLogs,
        },
      };
    }

    case 'SET_CONNECTION_STATUS': {
      return {
        ...state,
        connectionStatus: action.payload,
      };
    }

    case 'SELECT_PROJECT': {
      const nextSelectedProjectId =
        action.payload !== null && state.selectedProjectId === action.payload
          ? null
          : action.payload;

      return {
        ...state,
        selectedProjectId: nextSelectedProjectId,
        fileBrowseContext: nextSelectedProjectId === null ? null : state.fileBrowseContext,
      };
    }

    case 'CLEAR_LOGS': {
      return {
        ...state,
        logsByProjectId: {
          ...state.logsByProjectId,
          [action.payload]: [],
        },
      };
    }

    case 'SET_FILE_BROWSE_CONTEXT': {
      return {
        ...state,
        fileBrowseContext: action.payload,
      };
    }

    case 'SET_PROPOSAL_SESSIONS': {
      return {
        ...state,
        proposalSessionsByProjectId: {
          ...state.proposalSessionsByProjectId,
          [action.payload.projectId]: action.payload.sessions,
        },
      };
    }

    case 'ADD_PROPOSAL_SESSION': {
      const existing = state.proposalSessionsByProjectId[action.payload.projectId] || [];
      return {
        ...state,
        proposalSessionsByProjectId: {
          ...state.proposalSessionsByProjectId,
          [action.payload.projectId]: [...existing, action.payload.session],
        },
      };
    }

    case 'UPDATE_PROPOSAL_SESSION': {
      const session = action.payload;
      const projectSessions = state.proposalSessionsByProjectId[session.project_id] || [];
      return {
        ...state,
        proposalSessionsByProjectId: {
          ...state.proposalSessionsByProjectId,
          [session.project_id]: projectSessions.map((s) =>
            s.id === session.id ? session : s,
          ),
        },
      };
    }

    case 'REMOVE_PROPOSAL_SESSION': {
      const { projectId, sessionId } = action.payload;
      const sessions = state.proposalSessionsByProjectId[projectId] || [];
      return {
        ...state,
        proposalSessionsByProjectId: {
          ...state.proposalSessionsByProjectId,
          [projectId]: sessions.filter((s) => s.id !== sessionId),
        },
        activeProposalSessionId:
          state.activeProposalSessionId === sessionId ? null : state.activeProposalSessionId,
      };
    }

    case 'SET_ACTIVE_PROPOSAL_SESSION': {
      return {
        ...state,
        activeProposalSessionId: action.payload,
        // Clear elicitation when switching sessions
        activeElicitation: null,
      };
    }

    case 'HYDRATE_CHAT_MESSAGES': {
      const { sessionId, messages } = action.payload;
      return {
        ...state,
        chatMessagesBySessionId: {
          ...state.chatMessagesBySessionId,
          [sessionId]: messages,
        },
      };
    }

    case 'APPEND_CHAT_MESSAGE': {
      const { sessionId, message } = action.payload;
      const msgs = state.chatMessagesBySessionId[sessionId] || [];
      const existingIndex = msgs.findIndex((m) => m.id === message.id);
      const nextMessages =
        existingIndex === -1
          ? [...msgs, message]
          : msgs.map((existing, index) => (index === existingIndex ? message : existing));
      const nextStreamingContent = { ...state.streamingContent };
      if (message.role === 'assistant') {
        delete nextStreamingContent[message.id];
      }
      return {
        ...state,
        chatMessagesBySessionId: {
          ...state.chatMessagesBySessionId,
          [sessionId]: nextMessages,
        },
        streamingContent: nextStreamingContent,
      };
    }

    case 'START_ASSISTANT_TURN': {
      const { sessionId, messageId, turnId } = action.payload;
      return {
        ...state,
        activeTurnBySessionId: {
          ...state.activeTurnBySessionId,
          [sessionId]: {
            messageId,
            turnId,
            content: '',
            toolCalls: [],
            startedAt: new Date().toISOString(),
          },
        },
        isAgentResponding: true,
      };
    }

    case 'APPEND_STREAMING_CHUNK': {
      const { sessionId, messageId, content, turnId } = action.payload;
      const currentTurn = state.activeTurnBySessionId[sessionId];
      const targetTurn =
        currentTurn && currentTurn.messageId === messageId
          ? currentTurn
          : {
              messageId,
              turnId,
              content: '',
              toolCalls: [],
              startedAt: new Date().toISOString(),
            };
      return {
        ...state,
        activeTurnBySessionId: {
          ...state.activeTurnBySessionId,
          [sessionId]: {
            ...targetTurn,
            turnId: targetTurn.turnId ?? turnId,
            content: `${targetTurn.content}${content}`,
          },
        },
        streamingContent: {
          ...state.streamingContent,
          [messageId]: `${state.streamingContent[messageId] || ''}${content}`,
        },
        isAgentResponding: true,
      };
    }

    case 'COMPLETE_ASSISTANT_TURN': {
      const { sessionId } = action.payload;
      const activeTurn = state.activeTurnBySessionId[sessionId];
      if (!activeTurn) {
        return {
          ...state,
          isAgentResponding: false,
        };
      }
      const msgs = state.chatMessagesBySessionId[sessionId] || [];
      const assistantMessage: ProposalChatMessage = {
        id: activeTurn.messageId,
        role: 'assistant',
        content: activeTurn.content,
        timestamp: new Date().toISOString(),
        turn_id: activeTurn.turnId,
        tool_calls: activeTurn.toolCalls.length > 0 ? activeTurn.toolCalls : undefined,
      };
      const existingIndex = msgs.findIndex((m) => m.id === assistantMessage.id);
      const nextMessages =
        existingIndex === -1
          ? [...msgs, assistantMessage]
          : msgs.map((existing, index) => (index === existingIndex ? assistantMessage : existing));

      const nextActiveTurns = { ...state.activeTurnBySessionId };
      delete nextActiveTurns[sessionId];
      const nextStreamingContent = { ...state.streamingContent };
      delete nextStreamingContent[assistantMessage.id];

      return {
        ...state,
        chatMessagesBySessionId: {
          ...state.chatMessagesBySessionId,
          [sessionId]: nextMessages,
        },
        activeTurnBySessionId: nextActiveTurns,
        streamingContent: nextStreamingContent,
        isAgentResponding: false,
      };
    }

    case 'FAIL_ASSISTANT_TURN': {
      const { sessionId } = action.payload;
      const nextActiveTurns = { ...state.activeTurnBySessionId };
      const failedTurn = nextActiveTurns[sessionId];
      if (failedTurn) {
        delete nextActiveTurns[sessionId];
      }
      const nextStreamingContent = { ...state.streamingContent };
      if (failedTurn) {
        delete nextStreamingContent[failedTurn.messageId];
      }
      return {
        ...state,
        activeTurnBySessionId: nextActiveTurns,
        streamingContent: nextStreamingContent,
        isAgentResponding: false,
      };
    }

    case 'UPDATE_TOOL_CALL': {
      const { sessionId, messageId, turnId, toolCall } = action.payload;
      const activeTurn = state.activeTurnBySessionId[sessionId];
      if (activeTurn && activeTurn.messageId === messageId && (!turnId || activeTurn.turnId === turnId)) {
        const existingIndex = activeTurn.toolCalls.findIndex((tc) => tc.id === toolCall.id);
        const nextToolCalls =
          existingIndex === -1
            ? [...activeTurn.toolCalls, toolCall]
            : activeTurn.toolCalls.map((tc, index) => (index === existingIndex ? toolCall : tc));
        return {
          ...state,
          activeTurnBySessionId: {
            ...state.activeTurnBySessionId,
            [sessionId]: {
              ...activeTurn,
              turnId: activeTurn.turnId ?? turnId,
              toolCalls: nextToolCalls,
            },
          },
        };
      }

      const msgs = state.chatMessagesBySessionId[sessionId] || [];
      return {
        ...state,
        chatMessagesBySessionId: {
          ...state.chatMessagesBySessionId,
          [sessionId]: msgs.map((m) => {
            if (m.id !== messageId) return m;
            const existing = m.tool_calls || [];
            const existingIndex = existing.findIndex((tc) => tc.id === toolCall.id);
            const nextToolCalls =
              existingIndex === -1
                ? [...existing, toolCall]
                : existing.map((tc, index) => (index === existingIndex ? toolCall : tc));
            return { ...m, tool_calls: nextToolCalls };
          }),
        },
      };
    }

    case 'UPDATE_TOOL_CALL_STATUS': {
      const { sessionId, messageId, turnId, toolCallId, status } = action.payload;
      const activeTurn = state.activeTurnBySessionId[sessionId];
      if (activeTurn && activeTurn.messageId === messageId && (!turnId || activeTurn.turnId === turnId)) {
        return {
          ...state,
          activeTurnBySessionId: {
            ...state.activeTurnBySessionId,
            [sessionId]: {
              ...activeTurn,
              toolCalls: activeTurn.toolCalls.map((tc) =>
                tc.id === toolCallId ? { ...tc, status } : tc,
              ),
            },
          },
        };
      }

      const msgs = state.chatMessagesBySessionId[sessionId] || [];
      return {
        ...state,
        chatMessagesBySessionId: {
          ...state.chatMessagesBySessionId,
          [sessionId]: msgs.map((m) => {
            if (m.id !== messageId || !m.tool_calls) return m;
            return {
              ...m,
              tool_calls: m.tool_calls.map((tc) =>
                tc.id === toolCallId ? { ...tc, status } : tc,
              ),
            };
          }),
        },
      };
    }

    case 'SET_ELICITATION': {
      return {
        ...state,
        activeElicitation: action.payload,
      };
    }

    case 'SET_AGENT_RESPONDING': {
      return {
        ...state,
        isAgentResponding: action.payload,
      };
    }

    default:
      return state;
  }
}

export function useAppStore() {
  const [state, dispatch] = useReducer(appReducer, initialState);

  const setFullState = useCallback((fullState: FullState) => {
    dispatch({ type: 'SET_FULL_STATE', payload: fullState });
  }, []);

  const appendLog = useCallback((logEntry: RemoteLogEntry) => {
    dispatch({ type: 'APPEND_LOG', payload: logEntry });
  }, []);

  const setConnectionStatus = useCallback((status: ConnectionStatus) => {
    dispatch({ type: 'SET_CONNECTION_STATUS', payload: status });
  }, []);

  const selectProject = useCallback((projectId: string | null) => {
    dispatch({ type: 'SELECT_PROJECT', payload: projectId });
  }, []);

  const clearLogs = useCallback((projectId: string) => {
    dispatch({ type: 'CLEAR_LOGS', payload: projectId });
  }, []);

  const setWorktrees = useCallback((projectId: string, worktrees: WorktreeInfo[]) => {
    dispatch({ type: 'SET_WORKTREES', payload: { projectId, worktrees } });
  }, []);

  const setFileBrowseContext = useCallback((ctx: FileBrowseContext | null) => {
    dispatch({ type: 'SET_FILE_BROWSE_CONTEXT', payload: ctx });
  }, []);

  const setProposalSessions = useCallback((projectId: string, sessions: ProposalSession[]) => {
    dispatch({ type: 'SET_PROPOSAL_SESSIONS', payload: { projectId, sessions } });
  }, []);

  const addProposalSession = useCallback((projectId: string, session: ProposalSession) => {
    dispatch({ type: 'ADD_PROPOSAL_SESSION', payload: { projectId, session } });
  }, []);

  const updateProposalSession = useCallback((session: ProposalSession) => {
    dispatch({ type: 'UPDATE_PROPOSAL_SESSION', payload: session });
  }, []);

  const removeProposalSession = useCallback((projectId: string, sessionId: string) => {
    dispatch({ type: 'REMOVE_PROPOSAL_SESSION', payload: { projectId, sessionId } });
  }, []);

  const setActiveProposalSession = useCallback((sessionId: string | null) => {
    dispatch({ type: 'SET_ACTIVE_PROPOSAL_SESSION', payload: sessionId });
  }, []);

  const hydrateChatMessages = useCallback((sessionId: string, messages: ProposalChatMessage[]) => {
    dispatch({ type: 'HYDRATE_CHAT_MESSAGES', payload: { sessionId, messages } });
  }, []);

  const appendChatMessage = useCallback((sessionId: string, message: ProposalChatMessage) => {
    dispatch({ type: 'APPEND_CHAT_MESSAGE', payload: { sessionId, message } });
  }, []);

  const startAssistantTurn = useCallback((sessionId: string, messageId: string, turnId?: string) => {
    dispatch({ type: 'START_ASSISTANT_TURN', payload: { sessionId, messageId, turnId } });
  }, []);

  const appendStreamingChunk = useCallback((sessionId: string, messageId: string, content: string, turnId?: string) => {
    dispatch({ type: 'APPEND_STREAMING_CHUNK', payload: { sessionId, messageId, content, turnId } });
  }, []);

  const completeAssistantTurn = useCallback((sessionId: string, stopReason?: string) => {
    dispatch({ type: 'COMPLETE_ASSISTANT_TURN', payload: { sessionId, stopReason } });
  }, []);

  const failAssistantTurn = useCallback((sessionId: string, error: string) => {
    dispatch({ type: 'FAIL_ASSISTANT_TURN', payload: { sessionId, error } });
  }, []);

  const updateToolCall = useCallback((sessionId: string, messageId: string, toolCall: ToolCallInfo, turnId?: string) => {
    dispatch({ type: 'UPDATE_TOOL_CALL', payload: { sessionId, messageId, turnId, toolCall } });
  }, []);

  const updateToolCallStatus = useCallback((sessionId: string, messageId: string, toolCallId: string, status: ToolCallStatus, turnId?: string) => {
    dispatch({ type: 'UPDATE_TOOL_CALL_STATUS', payload: { sessionId, messageId, turnId, toolCallId, status } });
  }, []);

  const setElicitation = useCallback((elicitation: ElicitationRequest | null) => {
    dispatch({ type: 'SET_ELICITATION', payload: elicitation });
  }, []);

  const setAgentResponding = useCallback((responding: boolean) => {
    dispatch({ type: 'SET_AGENT_RESPONDING', payload: responding });
  }, []);

  return {
    state,
    setFullState,
    appendLog,
    setConnectionStatus,
    selectProject,
    clearLogs,
    setWorktrees,
    setFileBrowseContext,
    setProposalSessions,
    addProposalSession,
    updateProposalSession,
    removeProposalSession,
    setActiveProposalSession,
    hydrateChatMessages,
    appendChatMessage,
    startAssistantTurn,
    appendStreamingChunk,
    completeAssistantTurn,
    failAssistantTurn,
    updateToolCall,
    updateToolCallStatus,
    setElicitation,
    setAgentResponding,
  };
}
