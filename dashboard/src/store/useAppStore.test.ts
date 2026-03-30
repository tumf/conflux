/**
 * @vitest-environment jsdom
 */

import { describe, it, expect } from 'vitest';
import { FullState, RemoteLogEntry, RemoteProject } from '../api/types';
import { appReducer, AppState, AppAction } from './useAppStore';

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
  chatMessagesBySessionId: {},
  activeTurnBySessionId: {},
  activeElicitation: null,
  isAgentResponding: false,
  streamingContent: {},
  activeCommands: [],
  ...overrides,
});

describe('useAppStore - SET_FULL_STATE', () => {
  it('should replace projects while preserving selection and connection state', () => {
    const initialState = createState({
      projects: [createProject('old-project')],
      selectedProjectId: 'old-project',
      connectionStatus: 'connected',
    });

    const fullState: FullState = {
      projects: [createProject('new-project')],
      changes: [],
    };

    const action: AppAction = {
      type: 'SET_FULL_STATE',
      payload: fullState,
    };

    const state = appReducer(initialState, action);

    expect(state.projects).toHaveLength(1);
    expect(state.projects[0].id).toBe('new-project');
    expect(state.selectedProjectId).toBe('old-project');
    expect(state.connectionStatus).toBe('connected');
  });

  it('should clear projects when empty FullState is set', () => {
    const initialState = createState({
      projects: [createProject('project-1')],
      selectedProjectId: 'project-1',
      connectionStatus: 'connected',
    });

    const action: AppAction = {
      type: 'SET_FULL_STATE',
      payload: {
        projects: [],
        changes: [],
      },
    };

    const state = appReducer(initialState, action);

    expect(state.projects).toHaveLength(0);
  });

  it('should merge worktrees and orchestration metadata from full state', () => {
    const state = appReducer(createState(), {
      type: 'SET_FULL_STATE',
      payload: {
        projects: [createProject('project-1')],
        changes: [],
        worktrees: {
          'project-1': [
            {
              path: '/tmp/project-1',
              head: 'abc123',
              branch: 'feature/test',
              is_detached: false,
              is_main: false,
              merge_conflict: null,
              has_commits_ahead: false,
              is_merging: false,
            },
          ],
        },
        sync_available: true,
        orchestration_status: 'running',
        active_commands: [
          {
            project_id: 'project-1',
            root: 'base',
            operation: 'sync',
            started_at: '2026-03-29T00:00:00.000Z',
          },
        ],
      },
    });

    expect(state.worktreesByProjectId['project-1']).toHaveLength(1);
    expect(state.syncAvailable).toBe(true);
    expect(state.orchestrationStatus).toBe('running');
    expect(state.activeCommands).toHaveLength(1);
  });
});

describe('useAppStore - APPEND_LOG', () => {
  it('should append log entry to project logs', () => {
    const state = appReducer(createState(), {
      type: 'APPEND_LOG',
      payload: createLogEntry('project-1', 'Test log', '2026-03-29T00:00:00.000Z'),
    });

    expect(state.logsByProjectId['project-1']).toHaveLength(1);
    expect(state.logsByProjectId['project-1'][0].message).toBe('Test log');
  });

  it('should ignore log entries without project ids', () => {
    const state = appReducer(createState(), {
      type: 'APPEND_LOG',
      payload: {
        ...createLogEntry('project-1', 'Ignored log', '2026-03-29T00:00:00.000Z'),
        project_id: null,
      },
    });

    expect(state.logsByProjectId).toEqual({});
  });

  it('should trim logs to 500 entries per project', () => {
    const initialLogs = Array.from({ length: 500 }, (_, index) =>
      createLogEntry('project-1', `Log ${index}`, `2026-03-29T00:00:${String(index % 60).padStart(2, '0')}.000Z`),
    );

    const state = appReducer(
      createState({
        logsByProjectId: {
          'project-1': initialLogs,
        },
      }),
      {
        type: 'APPEND_LOG',
        payload: createLogEntry('project-1', 'Newest log', '2026-03-29T01:00:00.000Z'),
      },
    );

    expect(state.logsByProjectId['project-1']).toHaveLength(500);
    expect(state.logsByProjectId['project-1'][0].message).toBe('Log 1');
    expect(state.logsByProjectId['project-1'][499].message).toBe('Newest log');
  });
});

describe('useAppStore - SELECT_PROJECT', () => {
  it('should select a project when a different project is chosen', () => {
    const state = appReducer(createState(), {
      type: 'SELECT_PROJECT',
      payload: 'project-123',
    });

    expect(state.selectedProjectId).toBe('project-123');
  });

  it('should clear selection when the same project is chosen again', () => {
    const state = appReducer(
      createState({
        selectedProjectId: 'project-123',
        fileBrowseContext: { type: 'change', changeId: 'change-1' },
      }),
      {
        type: 'SELECT_PROJECT',
        payload: 'project-123',
      },
    );

    expect(state.selectedProjectId).toBeNull();
    expect(state.fileBrowseContext).toBeNull();
  });

  it('should support explicit deselection by null payload', () => {
    const state = appReducer(
      createState({
        selectedProjectId: 'project-123',
        fileBrowseContext: { type: 'worktree', worktreeBranch: 'feature/test' },
      }),
      {
        type: 'SELECT_PROJECT',
        payload: null,
      },
    );

    expect(state.selectedProjectId).toBeNull();
    expect(state.fileBrowseContext).toBeNull();
  });
});

describe('useAppStore - proposal chat state transitions', () => {
  it('keeps sequential assistant turns as distinct messages', () => {
    const sessionId = 'session-1';

    let state = appReducer(createState(), {
      type: 'START_ASSISTANT_TURN',
      payload: { sessionId, messageId: 'assistant-1', turnId: 'turn-1' },
    });

    state = appReducer(state, {
      type: 'APPEND_STREAMING_CHUNK',
      payload: { sessionId, messageId: 'assistant-1', content: 'Hello ', turnId: 'turn-1' },
    });

    state = appReducer(state, {
      type: 'APPEND_STREAMING_CHUNK',
      payload: { sessionId, messageId: 'assistant-1', content: 'world', turnId: 'turn-1' },
    });

    state = appReducer(state, {
      type: 'UPDATE_TOOL_CALL',
      payload: {
        sessionId,
        messageId: 'assistant-1',
        turnId: 'turn-1',
        toolCall: { id: 'tool-1', title: 'Lookup', status: 'pending' },
      },
    });

    state = appReducer(state, {
      type: 'COMPLETE_ASSISTANT_TURN',
      payload: { sessionId, stopReason: 'completed' },
    });

    state = appReducer(state, {
      type: 'START_ASSISTANT_TURN',
      payload: { sessionId, messageId: 'assistant-2', turnId: 'turn-2' },
    });

    state = appReducer(state, {
      type: 'APPEND_STREAMING_CHUNK',
      payload: { sessionId, messageId: 'assistant-2', content: 'Second reply', turnId: 'turn-2' },
    });

    state = appReducer(state, {
      type: 'COMPLETE_ASSISTANT_TURN',
      payload: { sessionId, stopReason: 'completed' },
    });

    const messages = state.chatMessagesBySessionId[sessionId] || [];
    expect(messages).toHaveLength(2);
    expect(messages[0]).toMatchObject({ id: 'assistant-1', role: 'assistant', content: 'Hello world', turn_id: 'turn-1' });
    expect(messages[0].tool_calls).toEqual([{ id: 'tool-1', title: 'Lookup', status: 'pending' }]);
    expect(messages[1]).toMatchObject({ id: 'assistant-2', role: 'assistant', content: 'Second reply', turn_id: 'turn-2' });
    expect(state.activeTurnBySessionId[sessionId]).toBeUndefined();
    expect(state.isAgentResponding).toBe(false);
  });

  it('hydrates history for an existing session', () => {
    const sessionId = 'session-1';
    const hydratedMessages = [
      {
        id: 'user-1',
        role: 'user' as const,
        content: 'hello',
        timestamp: '2026-03-29T00:00:00.000Z',
      },
      {
        id: 'assistant-1',
        role: 'assistant' as const,
        content: 'hi there',
        timestamp: '2026-03-29T00:00:01.000Z',
        hydrated: true,
      },
    ];

    const state = appReducer(createState(), {
      type: 'HYDRATE_CHAT_MESSAGES',
      payload: { sessionId, messages: hydratedMessages },
    });

    expect(state.chatMessagesBySessionId[sessionId]).toEqual(hydratedMessages);
  });

  it('clears active turn and re-enables input when turn fails', () => {
    const sessionId = 'session-1';

    let state = appReducer(createState(), {
      type: 'START_ASSISTANT_TURN',
      payload: { sessionId, messageId: 'assistant-1', turnId: 'turn-1' },
    });

    state = appReducer(state, {
      type: 'APPEND_STREAMING_CHUNK',
      payload: { sessionId, messageId: 'assistant-1', content: 'partial', turnId: 'turn-1' },
    });

    state = appReducer(state, {
      type: 'FAIL_ASSISTANT_TURN',
      payload: { sessionId, error: 'network' },
    });

    expect(state.activeTurnBySessionId[sessionId]).toBeUndefined();
    expect(state.streamingContent['assistant-1']).toBeUndefined();
    expect(state.isAgentResponding).toBe(false);
  });

  it('upserts server user message and marks it as sent', () => {
    const sessionId = 'session-1';
    const pendingMessage = {
      id: 'msg-1',
      role: 'user' as const,
      content: 'queued',
      timestamp: '2026-03-30T00:00:00.000Z',
      sendStatus: 'pending' as const,
    };

    let state = appReducer(createState(), {
      type: 'APPEND_CHAT_MESSAGE',
      payload: { sessionId, message: pendingMessage },
    });

    state = appReducer(state, {
      type: 'UPSERT_SERVER_USER_MESSAGE',
      payload: {
        sessionId,
        message: {
          id: 'msg-1',
          content: 'queued',
          timestamp: '2026-03-30T00:00:02.000Z',
        },
      },
    });

    const messages = state.chatMessagesBySessionId[sessionId] || [];
    expect(messages).toHaveLength(1);
    expect(messages[0]).toMatchObject({
      id: 'msg-1',
      role: 'user',
      content: 'queued',
      sendStatus: 'sent',
      hydrated: true,
      timestamp: '2026-03-30T00:00:02.000Z',
    });
  });

  it('updates user message send status to failed for retry UI', () => {
    const sessionId = 'session-1';
    const message = {
      id: 'msg-2',
      role: 'user' as const,
      content: 'hello',
      timestamp: '2026-03-30T00:00:00.000Z',
      sendStatus: 'pending' as const,
    };

    let state = appReducer(createState(), {
      type: 'APPEND_CHAT_MESSAGE',
      payload: { sessionId, message },
    });

    state = appReducer(state, {
      type: 'UPDATE_CHAT_MESSAGE_SEND_STATUS',
      payload: {
        sessionId,
        messageId: 'msg-2',
        sendStatus: 'failed',
      },
    });

    expect(state.chatMessagesBySessionId[sessionId]?.[0].sendStatus).toBe('failed');
  });
});

describe('useAppStore - ancillary actions', () => {
  it('should update connection status', () => {
    const state = appReducer(createState(), {
      type: 'SET_CONNECTION_STATUS',
      payload: 'connected',
    });

    expect(state.connectionStatus).toBe('connected');
  });

  it('should replace worktrees for a project', () => {
    const state = appReducer(createState(), {
      type: 'SET_WORKTREES',
      payload: {
        projectId: 'project-1',
        worktrees: [
          {
            path: '/tmp/project-1',
            head: 'abc123',
            branch: 'feature/test',
            is_detached: false,
            is_main: false,
            merge_conflict: null,
            has_commits_ahead: false,
            is_merging: false,
          },
        ],
      },
    });

    expect(state.worktreesByProjectId['project-1']).toHaveLength(1);
  });

  it('should clear logs for a specific project', () => {
    const state = appReducer(
      createState({
        logsByProjectId: {
          'project-1': [createLogEntry('project-1', 'Test log', '2026-03-29T00:00:00.000Z')],
        },
      }),
      {
        type: 'CLEAR_LOGS',
        payload: 'project-1',
      },
    );

    expect(state.logsByProjectId['project-1']).toEqual([]);
  });

  it('should update file browse context', () => {
    const state = appReducer(createState(), {
      type: 'SET_FILE_BROWSE_CONTEXT',
      payload: { type: 'change', changeId: 'change-1' },
    });

    expect(state.fileBrowseContext).toEqual({ type: 'change', changeId: 'change-1' });
  });
});
