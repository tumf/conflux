// @vitest-environment jsdom

import React from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, cleanup } from '@testing-library/react';

import { ChatInput } from '../ChatInput';

afterEach(() => {
  cleanup();
});

describe('ChatInput', () => {
  it('preserves newline on Enter and sends on Shift+Enter', () => {
    const onSend = vi.fn();

    render(<ChatInput onSend={onSend} status="ready" />);

    const textarea = screen.getByPlaceholderText('Type a message...') as HTMLTextAreaElement;

    fireEvent.change(textarea, { target: { value: 'hello' } });
    fireEvent.keyDown(textarea, { key: 'Enter' });

    expect(onSend).not.toHaveBeenCalled();

    fireEvent.change(textarea, { target: { value: 'line1' } });
    fireEvent.keyDown(textarea, { key: 'Enter', shiftKey: true });

    expect(onSend).toHaveBeenCalledTimes(1);
    expect(onSend).toHaveBeenCalledWith('line1');
  });

  it('keeps textarea enabled while send button is disabled during non-ready states', () => {
    const onSend = vi.fn();

    render(<ChatInput onSend={onSend} status="streaming" />);

    const textarea = screen.getByRole('textbox') as HTMLTextAreaElement;
    const sendButton = screen.getByLabelText('Send message') as HTMLButtonElement;

    expect(textarea.disabled).toBe(false);
    expect(sendButton.disabled).toBe(true);
  });

  it('keeps input value after sending', () => {
    const onSend = vi.fn();

    render(<ChatInput onSend={onSend} status="ready" />);

    const textarea = screen.getByRole('textbox') as HTMLTextAreaElement;

    fireEvent.change(textarea, { target: { value: 'trim me ' } });
    fireEvent.click(screen.getByLabelText('Send message'));

    expect(onSend).toHaveBeenCalledWith('trim me');
    expect(textarea.value).toBe('trim me ');
  });
});
