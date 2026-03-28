/**
 * Application State Management using useReducer
 */

import { useReducer, useCallback } from 'react';
import {
  RemoteProject,
  RemoteChange,
  RemoteLogEntry,
  FullState,
} from '../api/types';
import { ConnectionStatus } from '../api/wsClient';

export interface AppState {
  projects: RemoteProject[];
  selectedProjectId: string | null;
  logsByProjectId: Record<string, RemoteLogEntry[]>;
  connectionStatus: ConnectionStatus;
  changes: RemoteChange[];
}

export type AppAction =
  | { type: 'SET_FULL_STATE'; payload: FullState }
  | { type: 'APPEND_LOG'; payload: RemoteLogEntry }
  | { type: 'SET_CONNECTION_STATUS'; payload: ConnectionStatus }
  | { type: 'SELECT_PROJECT'; payload: string | null }
  | { type: 'CLEAR_LOGS'; payload: string };

const initialState: AppState = {
  projects: [],
  selectedProjectId: null,
  logsByProjectId: {},
  connectionStatus: 'disconnected',
  changes: [],
};

function appReducer(state: AppState, action: AppAction): AppState {
  switch (action.type) {
    case 'SET_FULL_STATE': {
      return {
        ...state,
        projects: action.payload.projects,
        changes: action.payload.changes,
      };
    }

    case 'APPEND_LOG': {
      const { project_id, ...logEntry } = action.payload;
      const logs = state.logsByProjectId[project_id] || [];
      const newLogs = [...logs, logEntry];
      // Keep only last 500 logs per project
      const trimmedLogs = newLogs.slice(-500);

      return {
        ...state,
        logsByProjectId: {
          ...state.logsByProjectId,
          [project_id]: trimmedLogs,
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

  return {
    state,
    setFullState,
    appendLog,
    setConnectionStatus,
    selectProject,
    clearLogs,
  };
}
