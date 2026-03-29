// @vitest-environment jsdom

import React from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';

import { ProposalChat } from '../ProposalChat';
import { ProposalSession } from '../../api/types';

vi.mock('../../hooks/useProposalWebSocket', () => ({
  useProposalWebSocket: () => ({
    status: 'disconnected',
    sendPrompt: vi.fn(),
    sendElicitationResponse: vi.fn(),
    sendCancel: vi.fn(),
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
        onStreamingChunk={vi.fn()}
        onToolCallStart={vi.fn()}
        onToolCallUpdate={vi.fn()}
        onElicitation={vi.fn()}
      />,
    );

    expect(screen.getByPlaceholderText('Connecting...')).toBeTruthy();
    expect(screen.getByTitle('Disconnected')).toBeTruthy();
  });
});
