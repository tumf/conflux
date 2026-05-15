/**
 * @vitest-environment jsdom
 */

import React from 'react';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import App from './App';
import { FullState, WorktreeInfo } from './api/types';

type Deferred<T> = {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (reason?: unknown) => void;
};

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const useWebSocketMock = vi.fn();
const deleteWorktreeMock = vi.fn();
const refreshWorktreesMock = vi.fn();
const listProposalSessionsMock = vi.fn();
const setUiStateMock = vi.fn();
const deleteUiStateMock = vi.fn();
const toastSuccessMock = vi.fn();
const toastErrorMock = vi.fn();

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

vi.mock('./api/restClient', () => ({
  APIError: class APIError extends Error {
    constructor(public status: number, message: string) {
      super(message);
    }
  },
  controlRun: vi.fn(),
  controlStop: vi.fn(),
  gitSync: vi.fn(),
  deleteProject: vi.fn(),
  addProject: vi.fn(),
  createWorktree: vi.fn(),
  deleteWorktree: (...args: unknown[]) => deleteWorktreeMock(...args),
  mergeWorktree: vi.fn(),
  refreshWorktrees: (...args: unknown[]) => refreshWorktreesMock(...args),
  createProposalSession: vi.fn(),
  listProposalSessions: (...args: unknown[]) => listProposalSessionsMock(...args),
  deleteProposalSession: vi.fn(),
  mergeProposalSession: vi.fn(),
  setUiState: (...args: unknown[]) => setUiStateMock(...args),
  deleteUiState: (...args: unknown[]) => deleteUiStateMock(...args),
}));

vi.mock('sonner', () => ({
  Toaster: () => null,
  toast: {
    success: (...args: unknown[]) => toastSuccessMock(...args),
    error: (...args: unknown[]) => toastErrorMock(...args),
  },
}));

vi.mock('./components/Header', () => ({ Header: () => <div data-testid="header" /> }));
vi.mock('./components/ProjectsPanel', () => ({ ProjectsPanel: () => <div data-testid="projects-panel" /> }));
vi.mock('./components/ChangesPanel', () => ({ ChangesPanel: () => <div data-testid="changes-panel" /> }));
vi.mock('./components/LogsPanel', () => ({ LogsPanel: () => <div data-testid="logs-panel" /> }));
vi.mock('./components/FileViewPanel', () => ({
  FileViewPanel: (props: { context: { type?: string } | null }) => (
    <div data-testid="file-view">{props.context?.type ?? 'none'}</div>
  ),
}));
vi.mock('./components/DeleteDialog', () => ({ DeleteDialog: () => null }));
vi.mock('./components/AddProjectDialog', () => ({ AddProjectDialog: () => null }));
vi.mock('./components/CreateWorktreeDialog', () => ({ CreateWorktreeDialog: () => null }));
vi.mock('./components/ProposalChat', () => ({ ProposalChat: () => <div data-testid="proposal-chat" /> }));
vi.mock('./components/ProposalSessionTabs', () => ({ ProposalSessionTabs: () => null }));
vi.mock('./components/CloseSessionDialog', () => ({ CloseSessionDialog: () => null }));
vi.mock('./components/OverviewDashboard', () => ({ OverviewDashboard: () => <div data-testid="overview" /> }));
vi.mock('./components/DeleteWorktreeDialog', () => ({
  DeleteWorktreeDialog: (props: {
    isOpen: boolean;
    branchName: string;
    onConfirm: () => void;
    onCancel: () => void;
    isLoading: boolean;
  }) => props.isOpen ? (
    <div data-testid="delete-worktree-dialog">
      <span>{props.branchName}</span>
      <span data-testid="dialog-loading">{String(props.isLoading)}</span>
      <button onClick={props.onConfirm}>confirm-delete-worktree</button>
      <button onClick={props.onCancel}>cancel-delete-worktree</button>
    </div>
  ) : null,
}));
vi.mock('./components/WorktreesPanel', () => ({
  WorktreesPanel: (props: {
    worktrees: WorktreeInfo[];
    deletingWorktreeBranch?: string | null;
    selectedWorktreeBranch?: string | null;
    onDelete: (branch: string) => void;
  }) => (
    <div data-testid="worktrees-panel">
      <div data-testid="deleting-worktree-branch">{props.deletingWorktreeBranch ?? 'none'}</div>
      <div data-testid="selected-worktree-branch">{props.selectedWorktreeBranch ?? 'none'}</div>
      {props.worktrees.map((worktree) => (
        <div key={worktree.branch} data-testid={`worktree-${worktree.branch}`}>
          {worktree.branch}
          <button onClick={() => props.onDelete(worktree.branch)}>delete-{worktree.branch}</button>
        </div>
      ))}
    </div>
  ),
}));

function makeWorktree(branch: string): WorktreeInfo {
  return {
    path: `/tmp/${branch}`,
    head: 'abc1234',
    branch,
    is_detached: false,
    is_main: false,
    merge_conflict: null,
    has_commits_ahead: false,
    is_merging: false,
  };
}

function makeState(uiState: Record<string, string> = {}): FullState {
  return {
    projects: [{
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
    }],
    changes: [],
    worktrees: {
      'project-1': [makeWorktree('feature-a'), makeWorktree('feature-b')],
    },
    ui_state: {
      selected_project_id: 'project-1',
      ...uiState,
    },
  };
}

async function renderHydratedApp(uiState: Record<string, string> = {}) {
  const user = userEvent.setup();
  render(<App />);
  latestWebSocketOptions.onStateUpdate?.(makeState(uiState));
  await waitFor(() => {
    expect(screen.getAllByText('Worktrees').length).toBeGreaterThan(0);
  });
  await user.click(screen.getAllByText('Worktrees')[0]);
  await waitFor(() => {
    expect(screen.getAllByText('delete-feature-a').length).toBeGreaterThan(0);
  });
}

describe('App worktree deletion pending state', () => {
  beforeEach(() => {
    latestWebSocketOptions = {};
    useWebSocketMock.mockClear();
    deleteWorktreeMock.mockReset();
    refreshWorktreesMock.mockReset();
    refreshWorktreesMock.mockResolvedValue([makeWorktree('feature-b')]);
    listProposalSessionsMock.mockReset();
    listProposalSessionsMock.mockResolvedValue([]);
    setUiStateMock.mockReset();
    setUiStateMock.mockResolvedValue(undefined);
    deleteUiStateMock.mockReset();
    deleteUiStateMock.mockResolvedValue(undefined);
    toastSuccessMock.mockClear();
    toastErrorMock.mockClear();
  });

  afterEach(() => {
    cleanup();
  });

  it('shows the deleting branch while delete is pending and clears browse context on success', async () => {
    const user = userEvent.setup();
    const deleteDeferred = deferred<void>();
    deleteWorktreeMock.mockReturnValue(deleteDeferred.promise);

    await renderHydratedApp({
      file_browse_context: JSON.stringify({ type: 'worktree', worktreeBranch: 'feature-a' }),
    });

    await user.click(screen.getAllByText('delete-feature-a')[0]);
    await user.click(screen.getByText('confirm-delete-worktree'));

    expect(deleteWorktreeMock).toHaveBeenCalledWith('project-1', 'feature-a');
    await waitFor(() => {
      expect(screen.getAllByTestId('deleting-worktree-branch').some((node) => node.textContent === 'feature-a')).toBe(true);
    });

    deleteDeferred.resolve(undefined);

    await waitFor(() => {
      expect(refreshWorktreesMock).toHaveBeenCalledWith('project-1');
      expect(toastSuccessMock).toHaveBeenCalledWith('Worktree deleted');
      expect(screen.getAllByTestId('deleting-worktree-branch').every((node) => node.textContent === 'none')).toBe(true);
    });
    expect(deleteUiStateMock).toHaveBeenCalledWith('file_browse_context');
    expect(screen.queryByTestId('delete-worktree-dialog')).toBeNull();
  });

  it('clears the deleting branch and leaves rows available when delete fails', async () => {
    const user = userEvent.setup();
    const deleteDeferred = deferred<void>();
    deleteWorktreeMock.mockReturnValue(deleteDeferred.promise);

    await renderHydratedApp();

    await user.click(screen.getAllByText('delete-feature-a')[0]);
    await user.click(screen.getByText('confirm-delete-worktree'));

    await waitFor(() => {
      expect(screen.getAllByTestId('deleting-worktree-branch').some((node) => node.textContent === 'feature-a')).toBe(true);
    });

    deleteDeferred.reject(new Error('delete failed'));

    await waitFor(() => {
      expect(toastErrorMock).toHaveBeenCalledWith('Failed to delete worktree: Error: delete failed');
      expect(screen.getAllByTestId('deleting-worktree-branch').every((node) => node.textContent === 'none')).toBe(true);
    });
    expect(refreshWorktreesMock).not.toHaveBeenCalled();
    expect(screen.getAllByTestId('worktree-feature-a').length).toBeGreaterThan(0);
  });

  it('passes the deleting branch through the mobile worktrees render path while pending', async () => {
    const user = userEvent.setup();
    const deleteDeferred = deferred<void>();
    deleteWorktreeMock.mockReturnValue(deleteDeferred.promise);

    await renderHydratedApp({
      mobile_active_tab: 'worktrees',
    });

    await user.click(screen.getAllByText('delete-feature-a')[0]);
    await user.click(screen.getByText('confirm-delete-worktree'));

    await waitFor(() => {
      const renderedWorktreesPanels = screen.getAllByTestId('deleting-worktree-branch');
      expect(renderedWorktreesPanels.length).toBeGreaterThanOrEqual(2);
      expect(renderedWorktreesPanels.every((node) => node.textContent === 'feature-a')).toBe(true);
    });

    deleteDeferred.resolve(undefined);

    await waitFor(() => {
      expect(screen.getAllByTestId('deleting-worktree-branch').every((node) => node.textContent === 'none')).toBe(true);
    });
  });
});
