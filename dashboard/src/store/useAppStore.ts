/**
 * Application State Management using useReducer
 */

import { useCallback, useReducer } from 'react';

import {
  ActiveCommand,
  FileBrowseContext,
  FullState,
  OrchestrationStatus,
  ProposalSession,
  RemoteLogEntry,
  RemoteProject,
  WorktreeInfo,
} from '../api/types';
import { ConnectionStatus } from '../api/wsClient';

export interface AppState {
  projects: RemoteProject[];
  selectedProjectId: string | null;
  logsByProjectId: Record<string, RemoteLogEntry[]>;
  connectionStatus: ConnectionStatus;
  worktreesByProjectId: Record<string, WorktreeInfo[]>;
  syncAvailable: boolean;
  orchestrationStatus: OrchestrationStatus;
  fileBrowseContext: FileBrowseContext | null;
  proposalSessionsByProjectId: Record<string, ProposalSession[]>;
  activeProposalSessionId: string | null;
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
  | { type: 'SET_ACTIVE_PROPOSAL_SESSION'; payload: string | null };

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
  activeCommands: [],
};

export function appReducer(state: AppState, action: AppAction): AppState {
  switch (action.type) {
    case 'SET_FULL_STATE': {
      const next: AppState = {
        ...state,
        projects: action.payload.projects,
        syncAvailable: action.payload.sync_available ?? false,
        orchestrationStatus: action.payload.orchestration_status ?? 'idle',
        activeCommands: action.payload.active_commands ?? [],
      };
      if (action.payload.worktrees) {
        next.worktreesByProjectId = {
          ...state.worktreesByProjectId,
          ...action.payload.worktrees,
        };
      }
      return next;
    }

    case 'APPEND_LOG': {
      const projectId = action.payload.project_id;
      if (!projectId) return state;

      const logs = state.logsByProjectId[projectId] || [];
      const newLogs = [...logs, action.payload].slice(-500);

      return {
        ...state,
        logsByProjectId: {
          ...state.logsByProjectId,
          [projectId]: newLogs,
        },
      };
    }

    case 'SET_CONNECTION_STATUS':
      return { ...state, connectionStatus: action.payload };

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

    case 'CLEAR_LOGS':
      return {
        ...state,
        logsByProjectId: {
          ...state.logsByProjectId,
          [action.payload]: [],
        },
      };

    case 'SET_WORKTREES':
      return {
        ...state,
        worktreesByProjectId: {
          ...state.worktreesByProjectId,
          [action.payload.projectId]: action.payload.worktrees,
        },
      };

    case 'SET_FILE_BROWSE_CONTEXT':
      return { ...state, fileBrowseContext: action.payload };

    case 'SET_PROPOSAL_SESSIONS':
      return {
        ...state,
        proposalSessionsByProjectId: {
          ...state.proposalSessionsByProjectId,
          [action.payload.projectId]: action.payload.sessions,
        },
      };

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
          [session.project_id]: projectSessions.map((s) => (s.id === session.id ? session : s)),
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

    case 'SET_ACTIVE_PROPOSAL_SESSION':
      return { ...state, activeProposalSessionId: action.payload };

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
  };
}
