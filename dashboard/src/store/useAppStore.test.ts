/**
 * Tests for useAppStore reducer
 */

import { describe, it, expect } from 'vitest';
import { appReducer, AppState, AppAction } from './useAppStore';

// Use a reducer directly for testing (extract from hook)
// Since appReducer is internal, we test it indirectly through dispatch behavior
describe('useAppStore - SET_FULL_STATE', () => {
  it('should set projects and changes from FullState', () => {
    const initialState: AppState = {
      projects: [],
      selectedProjectId: null,
      logsByProjectId: {},
      connectionStatus: 'disconnected',
      changes: [],
      worktreesByProjectId: {},
    };

    const fullState = {
      projects: [
        {
          id: 'test-project-1',
          name: 'repo@main',
          repo: 'repo',
          branch: 'main',
          status: 'idle' as const,
          is_busy: false,
          error: null,
          changes: [],
        },
      ],
      changes: [
        {
          id: 'change-1',
          project: 'test-project-1',
          status: 'idle' as const,
          completed_tasks: 0,
          total_tasks: 0,
          last_modified: '2024-01-01T00:00:00Z',
          iteration_number: null,
        },
      ],
    };

    const action: AppAction = {
      type: 'SET_FULL_STATE',
      payload: fullState,
    };

    const state = appReducer(initialState, action);

    expect(state.projects).toHaveLength(1);
    expect(state.projects[0].id).toBe('test-project-1');
    expect(state.changes).toHaveLength(1);
    expect(state.changes[0].id).toBe('change-1');
  });

  it('should replace projects when SET_FULL_STATE is dispatched', () => {
    const initialState: AppState = {
      projects: [
        {
          id: 'old-project',
          name: 'old-repo@main',
          repo: 'old-repo',
          branch: 'main',
          status: 'idle' as const,
          is_busy: false,
          error: null,
          changes: [],
        },
      ],
      selectedProjectId: 'old-project',
      logsByProjectId: {},
      connectionStatus: 'connected',
      changes: [],
      worktreesByProjectId: {},
    };

    const newFullState = {
      projects: [
        {
          id: 'new-project',
          name: 'new-repo@develop',
          repo: 'new-repo',
          branch: 'develop',
          status: 'running' as const,
          is_busy: true,
          error: null,
          changes: [],
        },
      ],
      changes: [],
    };

    const action: AppAction = {
      type: 'SET_FULL_STATE',
      payload: newFullState,
    };

    const state = appReducer(initialState, action);

    // Projects should be replaced
    expect(state.projects).toHaveLength(1);
    expect(state.projects[0].id).toBe('new-project');

    // Other state should remain unchanged
    expect(state.selectedProjectId).toBe('old-project');
    expect(state.connectionStatus).toBe('connected');
  });

  it('should clear projects when empty FullState is set', () => {
    const initialState: AppState = {
      projects: [
        {
          id: 'project-1',
          name: 'repo@main',
          repo: 'repo',
          branch: 'main',
          status: 'idle' as const,
          is_busy: false,
          error: null,
          changes: [],
        },
      ],
      selectedProjectId: 'project-1',
      logsByProjectId: {},
      connectionStatus: 'connected',
      changes: [],
      worktreesByProjectId: {},
    };

    const emptyFullState = {
      projects: [],
      changes: [],
    };

    const action: AppAction = {
      type: 'SET_FULL_STATE',
      payload: emptyFullState,
    };

    const state = appReducer(initialState, action);

    expect(state.projects).toHaveLength(0);
    expect(state.changes).toHaveLength(0);
  });
});

describe('useAppStore - APPEND_LOG', () => {
  it('should append log entry to project logs', () => {
    const initialState: AppState = {
      projects: [],
      selectedProjectId: null,
      logsByProjectId: {},
      connectionStatus: 'disconnected',
      changes: [],
      worktreesByProjectId: {},
    };

    const logEntry = {
      message: 'Test log',
      level: 'info' as const,
      change_id: null,
      timestamp: new Date().toISOString(),
      project_id: 'project-1',
      operation: null,
      iteration: null,
    };

    const action: AppAction = {
      type: 'APPEND_LOG',
      payload: logEntry,
    };

    const state = appReducer(initialState, action);

    expect(state.logsByProjectId['project-1']).toHaveLength(1);
    expect(state.logsByProjectId['project-1'][0].message).toBe('Test log');
  });

  it('should trim logs to 500 entries per project', () => {
    const initialState: AppState = {
      projects: [],
      selectedProjectId: null,
      logsByProjectId: {
        'project-1': Array.from({ length: 500 }, (_, i) => ({
          message: `Log ${i}`,
          level: 'info' as const,
          change_id: null,
          timestamp: new Date().toISOString(),
          project_id: 'project-1',
          operation: null,
          iteration: null,
        })),
      },
      connectionStatus: 'disconnected',
      changes: [],
      worktreesByProjectId: {},
    };

    const newLogEntry = {
      message: 'New log entry',
      level: 'info' as const,
      change_id: null,
      timestamp: new Date().toISOString(),
      project_id: 'project-1',
      operation: null,
      iteration: null,
    };

    const action: AppAction = {
      type: 'APPEND_LOG',
      payload: newLogEntry,
    };

    const state = appReducer(initialState, action);

    expect(state.logsByProjectId['project-1']).toHaveLength(500);
    expect(state.logsByProjectId['project-1'][499].message).toBe('New log entry');
  });
});

describe('useAppStore - SET_CONNECTION_STATUS', () => {
  it('should update connection status', () => {
    const initialState: AppState = {
      projects: [],
      selectedProjectId: null,
      logsByProjectId: {},
      connectionStatus: 'disconnected',
      changes: [],
      worktreesByProjectId: {},
    };

    const action: AppAction = {
      type: 'SET_CONNECTION_STATUS',
      payload: 'connected',
    };

    const state = appReducer(initialState, action);

    expect(state.connectionStatus).toBe('connected');
  });
});

describe('useAppStore - SELECT_PROJECT', () => {
  it('should select a project', () => {
    const initialState: AppState = {
      projects: [],
      selectedProjectId: null,
      logsByProjectId: {},
      connectionStatus: 'disconnected',
      changes: [],
      worktreesByProjectId: {},
    };

    const action: AppAction = {
      type: 'SELECT_PROJECT',
      payload: 'project-123',
    };

    const state = appReducer(initialState, action);

    expect(state.selectedProjectId).toBe('project-123');
  });

  it('should deselect by setting null', () => {
    const initialState: AppState = {
      projects: [],
      selectedProjectId: 'project-123',
      logsByProjectId: {},
      connectionStatus: 'disconnected',
      changes: [],
      worktreesByProjectId: {},
    };

    const action: AppAction = {
      type: 'SELECT_PROJECT',
      payload: null,
    };

    const state = appReducer(initialState, action);

    expect(state.selectedProjectId).toBeNull();
  });
});
