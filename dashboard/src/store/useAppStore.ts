/**
 * Application State Management using useReducer
 */

import { useReducer, useCallback } from 'react';
import {
  RemoteProject,
  RemoteChange,
  RemoteLogEntry,
  FullState,
  WorktreeInfo,
} from '../api/types';
import { ConnectionStatus } from '../api/wsClient';

export interface AppState {
  projects: RemoteProject[];
  selectedProjectId: string | null;
  logsByProjectId: Record<string, RemoteLogEntry[]>;
  connectionStatus: ConnectionStatus;
  changes: RemoteChange[];
  worktreesByProjectId: Record<string, WorktreeInfo[]>;
}

export type AppAction =
  | { type: 'SET_FULL_STATE'; payload: FullState }
  | { type: 'APPEND_LOG'; payload: RemoteLogEntry }
  | { type: 'SET_CONNECTION_STATUS'; payload: ConnectionStatus }
  | { type: 'SELECT_PROJECT'; payload: string | null }
  | { type: 'CLEAR_LOGS'; payload: string }
  | { type: 'SET_WORKTREES'; payload: { projectId: string; worktrees: WorktreeInfo[] } };

const initialState: AppState = {
  projects: [],
  selectedProjectId: null,
  logsByProjectId: {},
  connectionStatus: 'disconnected',
  changes: [],
  worktreesByProjectId: {},
};

export function appReducer(state: AppState, action: AppAction): AppState {
  switch (action.type) {
    case 'SET_FULL_STATE': {
      const newState: AppState = {
        ...state,
        projects: action.payload.projects,
        changes: action.payload.changes,
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
      return {
        ...state,
        selectedProjectId: action.payload,
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

  return {
    state,
    setFullState,
    appendLog,
    setConnectionStatus,
    selectProject,
    clearLogs,
    setWorktrees,
  };
}
