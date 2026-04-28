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
  RemoteChange,
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
  uiState: Record<string, string>;
  activeCommands: ActiveCommand[];
  optimisticChangeSelection: Record<string, boolean>;
  confirmedChangeSelection: Record<string, boolean>;
}

export type AppAction =
  | { type: 'SET_FULL_STATE'; payload: FullState }
  | { type: 'APPLY_CHANGE_UPDATE'; payload: RemoteChange }
  | { type: 'APPLY_OPTIMISTIC_CHANGE_SELECTION'; payload: { projectId: string; changeId: string; selected: boolean } }
  | { type: 'CLEAR_OPTIMISTIC_CHANGE_SELECTION'; payload: { projectId: string; changeId: string } }
  | { type: 'REVERT_OPTIMISTIC_CHANGE_SELECTION'; payload: { projectId: string; changeId: string } }
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
  uiState: {},
  activeCommands: [],
  optimisticChangeSelection: {},
  confirmedChangeSelection: {},
};

function changeSelectionKey(projectId: string, changeId: string): string {
  return `${projectId}::${changeId}`;
}

function applyOptimisticSelections(
  projects: RemoteProject[],
  optimisticSelection: Record<string, boolean>,
): RemoteProject[] {
  if (Object.keys(optimisticSelection).length === 0) {
    return projects;
  }

  return projects.map((project) => ({
    ...project,
    changes: project.changes.map((change) => {
      const key = changeSelectionKey(project.id, change.id);
      const optimisticValue = optimisticSelection[key];
      if (typeof optimisticValue !== 'boolean') {
        return change;
      }
      return {
        ...change,
        selected: optimisticValue,
      };
    }),
  }));
}

export function appReducer(state: AppState, action: AppAction): AppState {
  switch (action.type) {
    case 'SET_FULL_STATE': {
      const next: AppState = {
        ...state,
        projects: applyOptimisticSelections(action.payload.projects, state.optimisticChangeSelection),
        syncAvailable: action.payload.sync_available ?? false,
        orchestrationStatus: action.payload.orchestration_status ?? 'idle',
        uiState: action.payload.ui_state ?? {},
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

    case 'APPLY_CHANGE_UPDATE': {
      const incoming = action.payload;
      const key = changeSelectionKey(incoming.project, incoming.id);
      const shouldKeepOptimistic = key in state.optimisticChangeSelection;
      const nextProjects = state.projects.map((project) => {
        if (project.id !== incoming.project) {
          return project;
        }
        return {
          ...project,
          changes: project.changes.map((change) => {
            if (change.id !== incoming.id) {
              return change;
            }
            return {
              ...incoming,
              selected: shouldKeepOptimistic
                ? state.optimisticChangeSelection[key]
                : incoming.selected,
            };
          }),
        };
      });
      return {
        ...state,
        projects: nextProjects,
        confirmedChangeSelection: {
          ...state.confirmedChangeSelection,
          [key]: incoming.selected,
        },
      };
    }

    case 'APPLY_OPTIMISTIC_CHANGE_SELECTION': {
      const { projectId, changeId, selected } = action.payload;
      const key = changeSelectionKey(projectId, changeId);
      const nextOptimistic = {
        ...state.optimisticChangeSelection,
        [key]: selected,
      };
      const nextProjects = state.projects.map((project) => {
        if (project.id !== projectId) {
          return project;
        }
        return {
          ...project,
          changes: project.changes.map((change) =>
            change.id === changeId ? { ...change, selected } : change,
          ),
        };
      });
      return {
        ...state,
        projects: nextProjects,
        optimisticChangeSelection: nextOptimistic,
      };
    }

    case 'CLEAR_OPTIMISTIC_CHANGE_SELECTION': {
      const { projectId, changeId } = action.payload;
      const key = changeSelectionKey(projectId, changeId);
      if (!(key in state.optimisticChangeSelection)) {
        return state;
      }

      const { [key]: _, ...nextOptimistic } = state.optimisticChangeSelection;
      const hasConfirmedSelection = key in state.confirmedChangeSelection;
      const confirmedSelection = state.confirmedChangeSelection[key];
      const nextProjects = hasConfirmedSelection
        ? state.projects.map((project) => {
            if (project.id !== projectId) {
              return project;
            }
            return {
              ...project,
              changes: project.changes.map((change) =>
                change.id === changeId ? { ...change, selected: confirmedSelection } : change,
              ),
            };
          })
        : state.projects;
      return {
        ...state,
        projects: nextProjects,
        optimisticChangeSelection: nextOptimistic,
      };
    }

    case 'REVERT_OPTIMISTIC_CHANGE_SELECTION': {
      const { projectId, changeId } = action.payload;
      const key = changeSelectionKey(projectId, changeId);
      if (!(key in state.optimisticChangeSelection)) {
        return state;
      }

      const optimisticSelected = state.optimisticChangeSelection[key];
      const { [key]: _, ...nextOptimistic } = state.optimisticChangeSelection;
      const nextProjects = state.projects.map((project) => {
        if (project.id !== projectId) {
          return project;
        }
        return {
          ...project,
          changes: project.changes.map((change) => {
            if (change.id !== changeId) {
              return change;
            }
            return {
              ...change,
              selected: !optimisticSelected,
            };
          }),
        };
      });

      return {
        ...state,
        projects: nextProjects,
        optimisticChangeSelection: nextOptimistic,
      };
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

  const applyChangeUpdate = useCallback((change: RemoteChange) => {
    dispatch({ type: 'APPLY_CHANGE_UPDATE', payload: change });
  }, []);

  const applyOptimisticChangeSelection = useCallback(
    (projectId: string, changeId: string, selected: boolean) => {
      dispatch({
        type: 'APPLY_OPTIMISTIC_CHANGE_SELECTION',
        payload: { projectId, changeId, selected },
      });
    },
    [],
  );

  const clearOptimisticChangeSelection = useCallback((projectId: string, changeId: string) => {
    dispatch({
      type: 'CLEAR_OPTIMISTIC_CHANGE_SELECTION',
      payload: { projectId, changeId },
    });
  }, []);

  const revertOptimisticChangeSelection = useCallback((projectId: string, changeId: string) => {
    dispatch({
      type: 'REVERT_OPTIMISTIC_CHANGE_SELECTION',
      payload: { projectId, changeId },
    });
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
    applyChangeUpdate,
    applyOptimisticChangeSelection,
    clearOptimisticChangeSelection,
    revertOptimisticChangeSelection,
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
