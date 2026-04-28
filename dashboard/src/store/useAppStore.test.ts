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
  sync_state: 'up_to_date',
  ahead_count: 0,
  behind_count: 0,
  sync_required: false,
  local_sha: null,
  remote_sha: null,
  last_remote_check_at: null,
  remote_check_error: null,
  changes: [],
});

const createChange = (projectId: string, selected = false) => ({
  id: 'change-1',
  project: projectId,
  completed_tasks: 0,
  total_tasks: 2,
  last_modified: '2026-03-29T00:00:00.000Z',
  status: 'error' as const,
  iteration_number: null,
  selected,
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
  uiState: {},
  activeCommands: [],
  optimisticChangeSelection: {},
  confirmedChangeSelection: {},
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

  it('APPLY_OPTIMISTIC_CHANGE_SELECTION updates checkbox state immediately', () => {
    const project = createProject('project-1');
    project.changes = [createChange('project-1', false)];

    const state = appReducer(
      createState({ projects: [project] }),
      {
        type: 'APPLY_OPTIMISTIC_CHANGE_SELECTION',
        payload: { projectId: 'project-1', changeId: 'change-1', selected: true },
      },
    );

    expect(state.projects[0].changes[0].selected).toBe(true);
    expect(state.optimisticChangeSelection['project-1::change-1']).toBe(true);
  });

  it('APPLY_CHANGE_UPDATE keeps optimistic state until server reconciliation clears it', () => {
    const project = createProject('project-1');
    project.changes = [createChange('project-1', true)];

    const optimisticState = createState({
      projects: [project],
      optimisticChangeSelection: { 'project-1::change-1': true },
    });

    const withServerUpdate = appReducer(optimisticState, {
      type: 'APPLY_CHANGE_UPDATE',
      payload: { ...createChange('project-1', false), selected: false },
    });

    expect(withServerUpdate.projects[0].changes[0].selected).toBe(true);

    const reconciled = appReducer(withServerUpdate, {
      type: 'CLEAR_OPTIMISTIC_CHANGE_SELECTION',
      payload: { projectId: 'project-1', changeId: 'change-1' },
    });

    expect(reconciled.projects[0].changes[0].selected).toBe(false);
    expect(reconciled.optimisticChangeSelection['project-1::change-1']).toBeUndefined();
  });

  it('CLEAR_OPTIMISTIC_CHANGE_SELECTION keeps current value when no confirmed server value exists', () => {
    const project = createProject('project-1');
    project.changes = [createChange('project-1', true)];

    const optimisticState = createState({
      projects: [project],
      optimisticChangeSelection: { 'project-1::change-1': true },
    });

    const cleared = appReducer(optimisticState, {
      type: 'CLEAR_OPTIMISTIC_CHANGE_SELECTION',
      payload: { projectId: 'project-1', changeId: 'change-1' },
    });

    expect(cleared.projects[0].changes[0].selected).toBe(true);
    expect(cleared.optimisticChangeSelection['project-1::change-1']).toBeUndefined();
  });

  it('REVERT_OPTIMISTIC_CHANGE_SELECTION restores prior selection when toggle fails', () => {
    const project = createProject('project-1');
    project.changes = [createChange('project-1', true)];

    const state = appReducer(
      createState({
        projects: [project],
        optimisticChangeSelection: { 'project-1::change-1': true },
      }),
      {
        type: 'REVERT_OPTIMISTIC_CHANGE_SELECTION',
        payload: { projectId: 'project-1', changeId: 'change-1' },
      },
    );

    expect(state.projects[0].changes[0].selected).toBe(false);
    expect(state.optimisticChangeSelection['project-1::change-1']).toBeUndefined();
  });
});
