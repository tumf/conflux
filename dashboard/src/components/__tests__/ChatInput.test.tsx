// @vitest-environment jsdom

import React from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, cleanup } from '@testing-library/react';

import { ChatInput } from '../ChatInput';

afterEach(() => {
  cleanup();
});

describe('ChatInput', () => {
  it('sends on Enter and preserves multiline input with Shift+Enter', () => {
    const onSend = vi.fn();

    render(<ChatInput onSend={onSend} />);

    const textarea = screen.getByPlaceholderText('Type a message...') as HTMLTextAreaElement;

    fireEvent.change(textarea, { target: { value: 'hello' } });
    fireEvent.keyDown(textarea, { key: 'Enter' });

    expect(onSend).toHaveBeenCalledTimes(1);
    expect(onSend).toHaveBeenCalledWith('hello');

    fireEvent.keyDown(textarea, { key: 'Enter', shiftKey: true });

    expect(onSend).toHaveBeenCalledTimes(1);
  });

  it('locks textarea and send button while awaiting ACK', () => {
    const onSend = vi.fn();

    render(<ChatInput onSend={onSend} isSubmissionLocked />);

    const textarea = screen.getByRole('textbox') as HTMLTextAreaElement;
    const sendButton = screen.getByLabelText('Send message') as HTMLButtonElement;

    expect(textarea.disabled).toBe(true);
    expect(sendButton.disabled).toBe(true);
  });

  it('keeps input value after send and clears only when clearVersion increments', () => {
    const onSend = vi.fn();

    const { rerender } = render(<ChatInput onSend={onSend} isSubmissionLocked={false} clearVersion={0} />);

    const textarea = screen.getByRole('textbox') as HTMLTextAreaElement;

    fireEvent.change(textarea, { target: { value: 'trim me ' } });
    fireEvent.click(screen.getByLabelText('Send message'));

    expect(onSend).toHaveBeenCalledWith('trim me');
    expect(textarea.value).toBe('trim me ');

    rerender(<ChatInput onSend={onSend} isSubmissionLocked clearVersion={0} />);
    expect(textarea.value).toBe('trim me ');

    rerender(<ChatInput onSend={onSend} isSubmissionLocked={false} clearVersion={1} />);
    expect(textarea.value).toBe('');
  });
});
