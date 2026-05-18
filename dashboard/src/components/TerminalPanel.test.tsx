/**
 * @vitest-environment jsdom
 */

import React from 'react';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { TerminalPanel } from './TerminalPanel';
import { TerminalSessionInfo } from '../api/restClient';

const createTerminalSessionMock = vi.fn();
const deleteTerminalSessionMock = vi.fn();
const listTerminalSessionsMock = vi.fn();

vi.mock('../api/restClient', () => ({
  createTerminalSession: (...args: unknown[]) => createTerminalSessionMock(...args),
  deleteTerminalSession: (...args: unknown[]) => deleteTerminalSessionMock(...args),
  listTerminalSessions: (...args: unknown[]) => listTerminalSessionsMock(...args),
}));

vi.mock('./TerminalTab', () => ({
  TerminalTab: (props: { sessionId: string; isActive: boolean }) => (
    <div data-active={String(props.isActive)} data-testid={`terminal-tab-${props.sessionId}`} />
  ),
}));

function session(id: string, projectId: string, root: string): TerminalSessionInfo {
  return {
    id,
    cwd: `/tmp/${id}`,
    rows: 24,
    cols: 80,
    created_at: '2026-01-01T00:00:00Z',
    project_id: projectId,
    root,
  };
}

function renderPanel(props: Partial<React.ComponentProps<typeof TerminalPanel>> = {}) {
  return render(
    <TerminalPanel
      projectId="project-1"
      root="base"
      isExpanded
      onToggleExpand={vi.fn()}
      {...props}
    />,
  );
}

afterEach(() => {
  cleanup();
  createTerminalSessionMock.mockReset();
  deleteTerminalSessionMock.mockReset();
  listTerminalSessionsMock.mockReset();
});

describe('TerminalPanel session effects', () => {
  it('restores existing sessions and selects the first matching session for the current root', async () => {
    listTerminalSessionsMock.mockResolvedValue([
      session('other-root', 'project-1', 'worktree:feature-a'),
      session('base-a', 'project-1', 'base'),
      session('base-b', 'project-1', 'base'),
    ]);

    renderPanel();

    await waitFor(() => {
      expect(screen.getByTestId('terminal-tab-base-a').dataset.active).toBe('true');
    });

    expect(screen.getAllByText('base')).toHaveLength(2);
    expect(screen.getByTestId('terminal-tab-other-root').dataset.active).toBe('false');
    expect(createTerminalSessionMock).not.toHaveBeenCalled();
  });

  it('auto-creates a session once when the expanded panel has no matching restored session', async () => {
    const created = session('created-base', 'project-1', 'base');
    listTerminalSessionsMock.mockResolvedValue([]);
    createTerminalSessionMock.mockResolvedValue(created);

    renderPanel();

    await waitFor(() => {
      expect(createTerminalSessionMock).toHaveBeenCalledWith({
        project_id: 'project-1',
        root: 'base',
        rows: 24,
        cols: 80,
      });
    });

    await waitFor(() => {
      expect(screen.getByTestId('terminal-tab-created-base').dataset.active).toBe('true');
    });
    expect(createTerminalSessionMock).toHaveBeenCalledTimes(1);
  });

  it('switches the active tab when the root changes to another restored context', async () => {
    listTerminalSessionsMock.mockResolvedValue([
      session('base-a', 'project-1', 'base'),
      session('feature-a', 'project-1', 'worktree:feature-a'),
    ]);

    const rendered = renderPanel();

    await waitFor(() => {
      expect(screen.getByTestId('terminal-tab-base-a').dataset.active).toBe('true');
    });

    rendered.rerender(
      <TerminalPanel
        projectId="project-1"
        root="worktree:feature-a"
        isExpanded
        onToggleExpand={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(screen.getByTestId('terminal-tab-feature-a').dataset.active).toBe('true');
    });
    expect(screen.getByTestId('terminal-tab-base-a').dataset.active).toBe('false');
    expect(createTerminalSessionMock).not.toHaveBeenCalled();
  });
});
