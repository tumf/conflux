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
    };

    const fullState = {
      projects: [
        {
          id: 'test-project-1',
          remote_url: 'https://github.com/example/repo',
          branch: 'main',
          status: 'idle' as const,
          changes: [],
        },
      ],
      changes: [
        {
          id: 'change-1',
          project_id: 'test-project-1',
          status: 'idle' as const,
          completed_tasks: 0,
          total_tasks: 0,
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
          remote_url: 'https://github.com/old/repo',
          branch: 'main',
          status: 'idle',
          changes: [],
        },
      ],
      selectedProjectId: 'old-project',
      logsByProjectId: {},
      connectionStatus: 'connected',
      changes: [],
    };

    const newFullState = {
      projects: [
        {
          id: 'new-project',
          remote_url: 'https://github.com/new/repo',
          branch: 'develop',
          status: 'running',
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
          remote_url: 'https://github.com/example/repo',
          branch: 'main',
          status: 'idle',
          changes: [],
        },
      ],
      selectedProjectId: 'project-1',
      logsByProjectId: {},
      connectionStatus: 'connected',
      changes: [],
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
    };

    const logEntry = {
      project_id: 'project-1',
      timestamp: new Date().toISOString(),
      level: 'info' as const,
      message: 'Test log',
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
          timestamp: new Date().toISOString(),
          level: 'info' as const,
          message: `Log ${i}`,
        })),
      },
      connectionStatus: 'disconnected',
      changes: [],
    };

    const newLogEntry = {
      project_id: 'project-1',
      timestamp: new Date().toISOString(),
      level: 'info' as const,
      message: 'New log entry',
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
    };

    const action: AppAction = {
      type: 'SELECT_PROJECT',
      payload: null,
    };

    const state = appReducer(initialState, action);

    expect(state.selectedProjectId).toBeNull();
  });
});
