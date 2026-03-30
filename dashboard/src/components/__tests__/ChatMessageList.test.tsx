// @vitest-environment jsdom

import React from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';

import type { ProposalChatMessage } from '../../api/types';
import { ChatMessageList } from '../ChatMessageList';

if (!Element.prototype.scrollIntoView) {
  Element.prototype.scrollIntoView = vi.fn();
}

describe('ChatMessageList markdown rendering', () => {
  const writeText = vi.fn();

  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-03-30T00:10:00Z'));
    writeText.mockReset();
    Object.assign(navigator, {
      clipboard: {
        writeText,
      },
    });
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it('renders headings, lists, links, and horizontal rules', () => {
    const messages: ProposalChatMessage[] = [
      {
        id: 'm1',
        role: 'assistant',
        content: [
          '# Heading One',
          '## Heading Two',
          '### Heading Three',
          '- bullet one',
          '* bullet two',
          '1. first',
          '2. second',
          '[Conflux](https://example.com)',
          '---',
        ].join('\n'),
        timestamp: '2026-03-30T00:08:00Z',
      },
    ];

    const { container } = render(<ChatMessageList messages={messages} streamingContent={{}} />);

    expect(screen.getByRole('heading', { level: 1, name: 'Heading One' })).toBeTruthy();
    expect(screen.getByRole('heading', { level: 2, name: 'Heading Two' })).toBeTruthy();
    expect(screen.getByRole('heading', { level: 3, name: 'Heading Three' })).toBeTruthy();

    expect(container.querySelectorAll('ul li')).toHaveLength(2);
    expect(container.querySelectorAll('ol li')).toHaveLength(2);

    const link = screen.getByRole('link', { name: 'Conflux' });
    expect(link.getAttribute('href')).toBe('https://example.com');
    expect(link.getAttribute('target')).toBe('_blank');

    expect(container.querySelectorAll('hr')).toHaveLength(1);
  });

  it('renders code language label and copies code content', () => {
    const code = "console.log('ok')";
    const messages: ProposalChatMessage[] = [
      {
        id: 'm2',
        role: 'assistant',
        content: ['```typescript', code, '```'].join('\n'),
        timestamp: '2026-03-30T00:08:00Z',
      },
    ];

    render(<ChatMessageList messages={messages} streamingContent={{}} />);

    expect(screen.getByText('typescript')).toBeTruthy();
    const copyButtons = screen.getAllByRole('button', { name: 'Copy code' });
    fireEvent.click(copyButtons[0]);

    expect(writeText).toHaveBeenCalledWith(code);
  });

  it('copies assistant message and shows relative timestamp text', () => {
    const content = 'assistant message';
    const messages: ProposalChatMessage[] = [
      {
        id: 'm3',
        role: 'assistant',
        content,
        timestamp: '2026-03-30T00:08:00Z',
      },
      {
        id: 'm4',
        role: 'user',
        content: 'user message',
        timestamp: '2026-03-30T00:09:30Z',
      },
    ];

    render(<ChatMessageList messages={messages} streamingContent={{}} />);

    const messageCopy = screen.getByRole('button', { name: 'Copy message' });
    fireEvent.click(messageCopy);
    expect(writeText).toHaveBeenCalledWith(content);

    expect(screen.getByText('2 minutes ago')).toBeTruthy();
    expect(screen.getByText('30 seconds ago')).toBeTruthy();
  });
});
