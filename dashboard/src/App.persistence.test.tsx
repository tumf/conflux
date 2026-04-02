/**
 * @vitest-environment jsdom
 */

import React from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import App from './App';
import { FullState } from './api/types';

const useWebSocketMock = vi.fn();
const listProposalSessionsMock = vi.fn();
const setUiStateMock = vi.fn();
const deleteUiStateMock = vi.fn();

let latestWebSocketOptions: { onStateUpdate?: (state: FullState) => void } = {};

vi.mock('./hooks/useWebSocket', () => ({
  useWebSocket: (options: { onStateUpdate?: (state: FullState) => void }) => {
    latestWebSocketOptions = options;
    useWebSocketMock(options);
    return {
      isConnected: () => true,
      disconnect: () => undefined,
    };
  },
}));

vi.mock('./api/restClient', async () => {
  const actual = await vi.importActual<typeof import('./api/restClient')>('./api/restClient');
  return {
    ...actual,
    controlRun: vi.fn(),
    controlStop: vi.fn(),
    gitSync: vi.fn(),
    deleteProject: vi.fn(),
    addProject: vi.fn(),
    createWorktree: vi.fn(),
    deleteWorktree: vi.fn(),
    mergeWorktree: vi.fn(),
    refreshWorktrees: vi.fn().mockResolvedValue([]),
    createProposalSession: vi.fn(),
    listProposalSessions: (...args: unknown[]) => listProposalSessionsMock(...args),
    deleteProposalSession: vi.fn(),
    mergeProposalSession: vi.fn(),
    setUiState: (...args: unknown[]) => setUiStateMock(...args),
    deleteUiState: (...args: unknown[]) => deleteUiStateMock(...args),
  };
});

vi.mock('./components/Header', () => ({
  Header: () => <div data-testid="header" />,
}));

vi.mock('./components/ProjectsPanel', () => ({
  ProjectsPanel: () => <div data-testid="projects-panel" />,
}));

vi.mock('./components/ChangesPanel', () => ({
  ChangesPanel: (props: { selectedChangeId: string | null; onClickChange: (changeId: string) => void }) => (
    <div>
      <button onClick={() => props.onClickChange('change-a')}>select-change-a</button>
      <div data-testid="changes-selected">{props.selectedChangeId ?? 'none'}</div>
    </div>
  ),
}));

vi.mock('./components/WorktreesPanel', () => ({
  WorktreesPanel: (props: {
    selectedWorktreeBranch: string | null;
    onClickWorktree: (branch: string) => void;
  }) => (
    <div>
      <button onClick={() => props.onClickWorktree('feature-x')}>select-worktree-feature-x</button>
      <div data-testid="worktree-selected">{props.selectedWorktreeBranch ?? 'none'}</div>
    </div>
  ),
}));

vi.mock('./components/LogsPanel', () => ({
  LogsPanel: () => <div data-testid="logs-panel" />,
}));

vi.mock('./components/FileViewPanel', () => ({
  FileViewPanel: (props: { context: { type?: string } | null }) => (
    <div data-testid="file-view">{props.context?.type ?? 'none'}</div>
  ),
}));

vi.mock('./components/DeleteDialog', () => ({
  DeleteDialog: () => null,
}));

vi.mock('./components/DeleteWorktreeDialog', () => ({
  DeleteWorktreeDialog: () => null,
}));

vi.mock('./components/AddProjectDialog', () => ({
  AddProjectDialog: () => null,
}));

vi.mock('./components/CreateWorktreeDialog', () => ({
  CreateWorktreeDialog: () => null,
}));

vi.mock('./components/ProposalChat', () => ({
  ProposalChat: () => <div data-testid="proposal-chat" />,
}));

vi.mock('./components/ProposalSessionTabs', () => ({
  ProposalSessionTabs: () => null,
}));

vi.mock('./components/CloseSessionDialog', () => ({
  CloseSessionDialog: () => null,
}));

vi.mock('./components/OverviewDashboard', () => ({
  OverviewDashboard: () => <div data-testid="overview" />,
}));

describe('App ui-state persistence', () => {
  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    latestWebSocketOptions = {};
    useWebSocketMock.mockClear();
    listProposalSessionsMock.mockReset();
    listProposalSessionsMock.mockResolvedValue([]);
    setUiStateMock.mockReset();
    setUiStateMock.mockResolvedValue(undefined);
    deleteUiStateMock.mockReset();
    deleteUiStateMock.mockResolvedValue(undefined);
  });

  it('persists browse context and tab keys when user selects change/worktree', async () => {
    const user = userEvent.setup();
    render(<App />);

    latestWebSocketOptions.onStateUpdate?.({
      projects: [
        {
          id: 'project-1',
          name: 'repo@main',
          repo: 'repo',
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
          changes: [{
            id: 'change-a',
            project: 'project-1',
            completed_tasks: 0,
            total_tasks: 0,
            last_modified: '2026-04-01T00:00:00.000Z',
            status: 'idle',
            iteration_number: null,
            selected: false,
          }],
        },
      ],
      changes: [],
      worktrees: {
        'project-1': [{
          path: '/tmp/feature-x',
          head: 'abc1234',
          branch: 'feature-x',
          is_detached: false,
          is_main: false,
          merge_conflict: null,
          has_commits_ahead: false,
          is_merging: false,
        }],
      },
      ui_state: {
        selected_project_id: 'project-1',
      },
    });

    await waitFor(() => {
      expect(screen.getByText('select-change-a')).toBeTruthy();
    });

    setUiStateMock.mockClear();
    deleteUiStateMock.mockClear();

    await user.click(screen.getAllByText('select-change-a')[0]);

    expect(setUiStateMock).toHaveBeenCalledWith('file_browse_context', JSON.stringify({ type: 'change', changeId: 'change-a' }));
    expect(setUiStateMock).toHaveBeenCalledWith('desktop_center_tab', 'changes');
    expect(setUiStateMock).toHaveBeenCalledWith('desktop_right_tab', 'files');
    expect(setUiStateMock).toHaveBeenCalledWith('mobile_active_tab', 'files');

    await user.click(screen.getAllByText('Worktrees')[0]);
    await user.click(screen.getAllByText('select-worktree-feature-x')[0]);

    expect(setUiStateMock).toHaveBeenCalledWith('file_browse_context', JSON.stringify({ type: 'worktree', worktreeBranch: 'feature-x' }));
    expect(setUiStateMock).toHaveBeenCalledWith('desktop_center_tab', 'worktrees');
    expect(setUiStateMock).toHaveBeenCalledWith('desktop_right_tab', 'files');
    expect(setUiStateMock).toHaveBeenCalledWith('mobile_active_tab', 'files');
  });

  it('restores persisted change selection and files pane on hydration', async () => {
    render(<App />);

    latestWebSocketOptions.onStateUpdate?.({
      projects: [
        {
          id: 'project-1',
          name: 'repo@main',
          repo: 'repo',
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
          changes: [{
            id: 'change-a',
            project: 'project-1',
            completed_tasks: 0,
            total_tasks: 0,
            last_modified: '2026-04-01T00:00:00.000Z',
            status: 'idle',
            iteration_number: null,
            selected: false,
          }],
        },
      ],
      changes: [],
      worktrees: { 'project-1': [] },
      ui_state: {
        selected_project_id: 'project-1',
        file_browse_context: JSON.stringify({ type: 'change', changeId: 'change-a' }),
      },
    });

    await waitFor(() => {
      expect(screen.getByTestId('changes-selected').textContent).toBe('change-a');
      expect(screen.getAllByTestId('file-view').some((node) => node.textContent === 'change')).toBe(true);
    });
  });

  it('cleans stale persisted browse state without blocking startup', async () => {
    render(<App />);

    latestWebSocketOptions.onStateUpdate?.({
      projects: [
        {
          id: 'project-1',
          name: 'repo@main',
          repo: 'repo',
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
        },
      ],
      changes: [],
      worktrees: { 'project-1': [] },
      ui_state: {
        selected_project_id: 'project-1',
        file_browse_context: JSON.stringify({ type: 'change', changeId: 'stale-change' }),
      },
    });

    await waitFor(() => {
      expect(deleteUiStateMock).toHaveBeenCalledWith('file_browse_context');
      expect(deleteUiStateMock).toHaveBeenCalledWith('desktop_center_tab');
      expect(deleteUiStateMock).toHaveBeenCalledWith('desktop_right_tab');
      expect(deleteUiStateMock).toHaveBeenCalledWith('mobile_active_tab');
    });

    expect(screen.getAllByTestId('header').length).toBeGreaterThan(0);
    expect(screen.getByTestId('changes-selected').textContent).toBe('none');
  });
});
