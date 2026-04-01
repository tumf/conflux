/**
 * @vitest-environment jsdom
 */

import { describe, expect, it } from 'vitest';

import { FullState, RemoteLogEntry, RemoteProject } from '../api/types';
import { appReducer, AppAction, AppState } from './useAppStore';

const createProject = (id: string): RemoteProject => ({
  id,
  name: `${id}@main`,
  repo: id,
  branch: 'main',
  status: 'idle',
  is_busy: false,
  error: null,
  changes: [],
});

const createLogEntry = (projectId: string, message: string, timestamp: string): RemoteLogEntry => ({
  message,
  level: 'info',
  change_id: null,
  timestamp,
  project_id: projectId,
  operation: null,
  iteration: null,
});

const createState = (overrides: Partial<AppState> = {}): AppState => ({
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
  ...overrides,
});

describe('useAppStore reducer', () => {
  it('SET_FULL_STATE updates projects while preserving selection', () => {
    const initialState = createState({
      projects: [createProject('old-project')],
      selectedProjectId: 'old-project',
      connectionStatus: 'connected',
    });

    const fullState: FullState = {
      projects: [createProject('new-project')],
      changes: [],
    };

    const action: AppAction = { type: 'SET_FULL_STATE', payload: fullState };
    const state = appReducer(initialState, action);

    expect(state.projects).toHaveLength(1);
    expect(state.projects[0].id).toBe('new-project');
    expect(state.selectedProjectId).toBe('old-project');
    expect(state.connectionStatus).toBe('connected');
  });

  it('APPEND_LOG keeps last 500 logs', () => {
    const initialLogs = Array.from({ length: 500 }, (_, index) =>
      createLogEntry('project-1', `Log ${index}`, `2026-03-29T00:00:${String(index % 60).padStart(2, '0')}.000Z`),
    );

    const state = appReducer(
      createState({ logsByProjectId: { 'project-1': initialLogs } }),
      {
        type: 'APPEND_LOG',
        payload: createLogEntry('project-1', 'Newest log', '2026-03-29T01:00:00.000Z'),
      },
    );

    expect(state.logsByProjectId['project-1']).toHaveLength(500);
    expect(state.logsByProjectId['project-1'][0].message).toBe('Log 1');
    expect(state.logsByProjectId['project-1'][499].message).toBe('Newest log');
  });

  it('SELECT_PROJECT toggles same selection to null', () => {
    const state = appReducer(
      createState({ selectedProjectId: 'project-123' }),
      { type: 'SELECT_PROJECT', payload: 'project-123' },
    );

    expect(state.selectedProjectId).toBeNull();
  });
});
