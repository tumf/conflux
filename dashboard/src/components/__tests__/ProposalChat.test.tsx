// @vitest-environment jsdom

import React from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';

import { ProposalChat } from '../ProposalChat';
import { ProposalSession } from '../../api/types';

vi.mock('../../hooks/useProposalWebSocket', () => ({
  useProposalWebSocket: () => ({
    status: 'disconnected',
    sendPrompt: vi.fn(),
    sendElicitationResponse: vi.fn(),
  }),
}));

const listProposalSessionMessagesMock = vi.fn().mockResolvedValue({
  messages: [
    {
      id: 'assistant-hydrated',
      role: 'assistant',
      content: 'hydrated',
      timestamp: '2026-03-29T00:00:00.000Z',
    },
  ],
});

vi.mock('../../api/restClient', () => ({
  listProposalSessionMessages: (...args: unknown[]) => listProposalSessionMessagesMock(...args),
}));

vi.mock('../ProposalChangesList', () => ({
  ProposalChangesList: () => <div>changes</div>,
}));

vi.mock('../ProposalActions', () => ({
  ProposalActions: () => <div>actions</div>,
}));

afterEach(() => {
  cleanup();
  listProposalSessionMessagesMock.mockClear();
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
        onHydrateMessages={vi.fn()}
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

  it('hydrates chat history when mounting the session', async () => {
    const onHydrateMessages = vi.fn();

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
        onHydrateMessages={onHydrateMessages}
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

    await waitFor(() => {
      expect(onHydrateMessages).toHaveBeenCalledWith(
        'session-1',
        [
          expect.objectContaining({
            id: 'assistant-hydrated',
            hydrated: true,
          }),
        ],
      );
    });
  });
});
