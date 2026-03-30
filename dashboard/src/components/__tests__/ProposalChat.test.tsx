// @vitest-environment jsdom

import React from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';

import { ProposalChat } from '../ProposalChat';
import { ProposalSession } from '../../api/types';

vi.mock('../../hooks/useProposalWebSocket', () => ({
  useProposalWebSocket: () => ({
    status: 'disconnected',
    sendPrompt: vi.fn(),
    sendElicitationResponse: vi.fn(),
  }),
}));


vi.mock('../ProposalChangesList', () => ({
  ProposalChangesList: () => <div>changes</div>,
}));

vi.mock('../ProposalActions', () => ({
  ProposalActions: () => <div>actions</div>,
}));

afterEach(() => {
  cleanup();
});

if (!Element.prototype.scrollIntoView) {
  Element.prototype.scrollIntoView = vi.fn();
}

const session: ProposalSession = {
  id: 'session-1',
  project_id: 'project-1',
  status: 'timed_out',
  worktree_branch: 'proposal/session-1',
  is_dirty: false,
  uncommitted_files: [],
  created_at: '2026-03-29T00:00:00Z',
  updated_at: '2026-03-29T00:10:00Z',
};

describe('ProposalChat timeout handling', () => {
  it('shows reconnect-oriented disconnected state when websocket is unavailable after timeout', () => {
    render(
      <ProposalChat
        projectId="project-1"
        session={session}
        messages={[]}
        streamingContent={{}}
        activeElicitation={null}
        isAgentResponding={false}
        onBack={vi.fn()}
        onMerge={vi.fn()}
        onClose={vi.fn()}
        onAppendMessage={vi.fn()}
        onStartAssistantTurn={vi.fn()}
        onStreamingChunk={vi.fn()}
        onCompleteAssistantTurn={vi.fn()}
        onFailAssistantTurn={vi.fn()}
        onToolCallStart={vi.fn()}
        onToolCallUpdate={vi.fn()}
        onElicitation={vi.fn()}
      />,
    );

    expect(screen.getByPlaceholderText('Connecting...')).toBeTruthy();
    expect(screen.getByTitle('Disconnected')).toBeTruthy();
  });

  it('opens and closes changes drawer with button, backdrop, and Escape key', () => {
    render(
      <ProposalChat
        projectId="project-1"
        session={session}
        messages={[]}
        streamingContent={{}}
        activeElicitation={null}
        isAgentResponding={false}
        onBack={vi.fn()}
        onMerge={vi.fn()}
        onClose={vi.fn()}
        onAppendMessage={vi.fn()}
        onStartAssistantTurn={vi.fn()}
        onStreamingChunk={vi.fn()}
        onCompleteAssistantTurn={vi.fn()}
        onFailAssistantTurn={vi.fn()}
        onToolCallStart={vi.fn()}
        onToolCallUpdate={vi.fn()}
        onElicitation={vi.fn()}
      />,
    );

    const dialog = screen.getByRole('dialog', { hidden: true });
    expect(dialog.className).toContain('pointer-events-none');

    fireEvent.click(screen.getByLabelText('Open changes drawer'));
    expect(dialog.className).toContain('pointer-events-auto');

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(dialog.className).toContain('pointer-events-none');

    fireEvent.click(screen.getByLabelText('Open changes drawer'));
    expect(dialog.className).toContain('pointer-events-auto');

    fireEvent.click(dialog);
    expect(dialog.className).toContain('pointer-events-none');
  });
});
