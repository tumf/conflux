// @vitest-environment jsdom

import React from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';

import { ChatMessageList } from '../ChatMessageList';
import { ProposalChatMessage } from '../../api/types';

const scrollIntoViewMock = vi.fn();

function setScrollMetrics(container: HTMLElement, metrics: { scrollTop: number; clientHeight: number; scrollHeight: number }) {
  Object.defineProperty(container, 'scrollTop', {
    value: metrics.scrollTop,
    writable: true,
    configurable: true,
  });
  Object.defineProperty(container, 'clientHeight', {
    value: metrics.clientHeight,
    configurable: true,
  });
  Object.defineProperty(container, 'scrollHeight', {
    value: metrics.scrollHeight,
    configurable: true,
  });
}

const baseMessages: ProposalChatMessage[] = [
  {
    id: 'm1',
    role: 'user',
    content: 'hello',
    timestamp: '2026-03-30T00:00:00.000Z',
    hydrated: true,
  },
];

beforeEach(() => {
  scrollIntoViewMock.mockReset();
  Element.prototype.scrollIntoView = scrollIntoViewMock;
});

afterEach(() => {
  cleanup();
});

describe('ChatMessageList smart scroll', () => {
  it('does not auto-scroll and shows New messages when user scrolled up', () => {
    const { rerender } = render(
      <ChatMessageList
        messages={baseMessages}
        streamingContent={{}}
      />,
    );

    const scroller = screen.getByTestId('chat-scroll-container');
    setScrollMetrics(scroller, {
      scrollTop: 100,
      clientHeight: 400,
      scrollHeight: 1000,
    });

    // User scrolled up far from bottom
    scroller.dispatchEvent(new Event('scroll'));
    scrollIntoViewMock.mockClear();

    rerender(
      <ChatMessageList
        messages={baseMessages}
        streamingContent={{ stream1: 'new chunk' }}
      />,
    );

    expect(scrollIntoViewMock).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: '↓ New messages' })).toBeTruthy();
  });
});
