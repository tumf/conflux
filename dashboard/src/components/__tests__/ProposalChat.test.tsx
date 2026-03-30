// @vitest-environment jsdom

import React from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';

import { ProposalChat } from '../ProposalChat';
import { ProposalSession } from '../../api/types';

const sendPromptMock = vi.fn();
const sendElicitationResponseMock = vi.fn();
let wsStatus: 'connected' | 'disconnected' = 'disconnected';

vi.mock('../../hooks/useProposalWebSocket', () => ({
  useProposalWebSocket: () => ({
    status: wsStatus,
    sendPrompt: sendPromptMock,
    sendElicitationResponse: sendElicitationResponseMock,
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
  sendPromptMock.mockReset();
  sendElicitationResponseMock.mockReset();
  wsStatus = 'disconnected';
});

beforeEach(() => {
  if (!Element.prototype.scrollIntoView) {
    Element.prototype.scrollIntoView = vi.fn();
  }
});

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
        onUpsertServerUserMessage={vi.fn()}
        onUpdateMessageSendStatus={vi.fn()}
        onStartAssistantTurn={vi.fn()}
        onStreamingChunk={vi.fn()}
        onCompleteAssistantTurn={vi.fn()}
        onFailAssistantTurn={vi.fn()}
        onToolCallStart={vi.fn()}
        onToolCallUpdate={vi.fn()}
        onElicitation={vi.fn()}
      />,
    );

    expect(screen.getByPlaceholderText('Disconnected. Message will be queued and sent on reconnect.')).toBeTruthy();
    expect(screen.getByTitle('Disconnected')).toBeTruthy();
  });

  it('shows enter-to-send hint when connected', () => {
    wsStatus = 'connected';

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
        onUpsertServerUserMessage={vi.fn()}
        onUpdateMessageSendStatus={vi.fn()}
        onStartAssistantTurn={vi.fn()}
        onStreamingChunk={vi.fn()}
        onCompleteAssistantTurn={vi.fn()}
        onFailAssistantTurn={vi.fn()}
        onToolCallStart={vi.fn()}
        onToolCallUpdate={vi.fn()}
        onElicitation={vi.fn()}
      />,
    );

    expect(screen.getByPlaceholderText('Type a message... (Enter to send, Shift+Enter for newline)')).toBeTruthy();
  });

  it('sends example prompt when empty-state chip is clicked', () => {
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
        onUpsertServerUserMessage={vi.fn()}
        onUpdateMessageSendStatus={vi.fn()}
        onStartAssistantTurn={vi.fn()}
        onStreamingChunk={vi.fn()}
        onCompleteAssistantTurn={vi.fn()}
        onFailAssistantTurn={vi.fn()}
        onToolCallStart={vi.fn()}
        onToolCallUpdate={vi.fn()}
        onElicitation={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByText('Summarize the current proposal and open risks'));

    expect(sendPromptMock).toHaveBeenCalledTimes(1);
    expect(sendPromptMock.mock.calls[0][0]).toBe('Summarize the current proposal and open risks');
    expect(typeof sendPromptMock.mock.calls[0][1]).toBe('string');
  });
});
