// @vitest-environment jsdom

import React from 'react';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import { ChatInput } from '../ChatInput';

describe('ChatInput key bindings', () => {
  it('sends on Enter and preserves newline on Shift+Enter', () => {
    const onSend = vi.fn();

    render(<ChatInput onSend={onSend} />);

    const textarea = screen.getByPlaceholderText('Type a message...') as HTMLTextAreaElement;

    fireEvent.change(textarea, { target: { value: 'hello' } });
    fireEvent.keyDown(textarea, { key: 'Enter' });

    expect(onSend).toHaveBeenCalledWith('hello');

    fireEvent.change(textarea, { target: { value: 'line1' } });
    fireEvent.keyDown(textarea, { key: 'Enter', shiftKey: true });

    expect(onSend).toHaveBeenCalledTimes(1);
  });
});
