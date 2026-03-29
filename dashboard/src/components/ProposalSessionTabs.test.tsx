// @vitest-environment jsdom

import React from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { ProposalSessionTabs } from './ProposalSessionTabs';
import { ProposalSession } from '../api/types';

afterEach(() => {
  cleanup();
});

const sessions: ProposalSession[] = [
  {
    id: 'session-1',
    project_id: 'project-1',
    status: 'active',
    worktree_branch: 'proposal/alpha',
    is_dirty: false,
    uncommitted_files: [],
    created_at: '2026-03-29T00:00:00Z',
    updated_at: '2026-03-29T00:00:00Z',
  },
  {
    id: 'session-2',
    project_id: 'project-1',
    status: 'active',
    worktree_branch: 'proposal/beta',
    is_dirty: true,
    uncommitted_files: ['proposal.md'],
    created_at: '2026-03-29T00:00:00Z',
    updated_at: '2026-03-29T00:00:00Z',
  },
];

describe('ProposalSessionTabs', () => {
  it('supports selecting another session and creating a session', async () => {
    const onSelectSession = vi.fn();
    const onCreateSession = vi.fn();
    const onCloseSession = vi.fn();
    const user = userEvent.setup();

    const { rerender } = render(
      <ProposalSessionTabs
        sessions={sessions}
        activeSessionId="session-1"
        onSelectSession={onSelectSession}
        onCreateSession={onCreateSession}
        onCloseSession={onCloseSession}
      />,
    );

    const sessionButtons = screen.getAllByRole('button').filter((button) =>
      button.textContent?.includes('proposal/beta'),
    );

    await user.click(sessionButtons[0]);
    await user.click(screen.getByRole('button', { name: 'New proposal session' }));

    expect(onSelectSession).toHaveBeenCalledWith('session-2');
    expect(onCreateSession).toHaveBeenCalledTimes(1);

    rerender(
      <ProposalSessionTabs
        sessions={sessions}
        activeSessionId="session-2"
        onSelectSession={onSelectSession}
        onCreateSession={onCreateSession}
        onCloseSession={onCloseSession}
      />,
    );

    const rerenderedButtons = screen.getAllByRole('button').filter((button) =>
      button.textContent?.includes('proposal/beta'),
    );

    expect(rerenderedButtons[0].className).toContain('bg-[#1e1b4b]/50');
  });

  it('closes a session from the close control without selecting it', async () => {
    const onSelectSession = vi.fn();
    const onCreateSession = vi.fn();
    const onCloseSession = vi.fn();
    const user = userEvent.setup();

    render(
      <ProposalSessionTabs
        sessions={sessions}
        activeSessionId="session-1"
        onSelectSession={onSelectSession}
        onCreateSession={onCreateSession}
        onCloseSession={onCloseSession}
      />,
    );

    await user.click(screen.getByLabelText('Close session proposal/beta'));

    expect(onCloseSession).toHaveBeenCalledWith('session-2');
    expect(onSelectSession).not.toHaveBeenCalledWith('session-2');
  });
});
