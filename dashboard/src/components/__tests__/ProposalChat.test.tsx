// @vitest-environment jsdom

import React from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';

import { ProposalChat } from '../ProposalChat';

const sendMessageMock = vi.fn();
const stopMock = vi.fn();
const sendElicitationResponseMock = vi.fn();

let hookState: {
  messages: Array<{ id: string; role: 'user' | 'assistant'; content: string; timestamp: string }>;
  status: 'ready' | 'submitted' | 'streaming' | 'error';
  error: string | null;
  activeElicitation: null;
  wsConnected: boolean;
} = {
  messages: [],
  status: 'ready',
  error: null,
  activeElicitation: null,
  wsConnected: false,
};

vi.mock('../../hooks/useProposalChat', () => ({
  useProposalChat: () => ({
    ...hookState,
    sendMessage: sendMessageMock,
    stop: stopMock,
    sendElicitationResponse: sendElicitationResponseMock,
  }),
}));

vi.mock('../ProposalChangesList', () => ({
  ProposalChangesList: () => <div>changes</div>,
}));

vi.mock('../ProposalActions', () => ({
  ProposalActions: () => <div>actions</div>,
}));

beforeEach(() => {
  if (!Element.prototype.scrollIntoView) {
    Element.prototype.scrollIntoView = vi.fn();
  }
});

afterEach(() => {
  cleanup();
  sendMessageMock.mockReset();
  stopMock.mockReset();
  sendElicitationResponseMock.mockReset();
  hookState = {
    messages: [],
    status: 'ready',
    error: null,
    activeElicitation: null,
    wsConnected: false,
  };
});

describe('ProposalChat', () => {
  it('shows disconnected placeholder when websocket unavailable', () => {
    render(
      <ProposalChat
        projectId="project-1"
        sessionId="session-1"
        onBack={vi.fn()}
        onMerge={vi.fn()}
        onClose={vi.fn()}
      />,
    );

    expect(
      screen.getByPlaceholderText('Disconnected. Message will be queued and sent on reconnect.'),
    ).toBeTruthy();
    expect(screen.getByTitle('Disconnected')).toBeTruthy();
  });

  it('shows normal placeholder when connected and ready', () => {
    hookState.wsConnected = true;
    hookState.status = 'ready';

    render(
      <ProposalChat
        projectId="project-1"
        sessionId="session-1"
        onBack={vi.fn()}
        onMerge={vi.fn()}
        onClose={vi.fn()}
      />,
    );

    expect(
      screen.getByPlaceholderText('Type a message... (Enter to send, Shift+Enter for newline)'),
    ).toBeTruthy();
  });

  it('sends example prompt through hook', () => {
    render(
      <ProposalChat
        projectId="project-1"
        sessionId="session-1"
        onBack={vi.fn()}
        onMerge={vi.fn()}
        onClose={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByText('Summarize the current proposal and open risks'));

    expect(sendMessageMock).toHaveBeenCalledTimes(1);
    expect(sendMessageMock).toHaveBeenCalledWith('Summarize the current proposal and open risks');
  });
});
