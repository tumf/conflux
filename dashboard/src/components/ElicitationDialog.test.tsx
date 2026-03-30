// @vitest-environment jsdom

import React from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { ElicitationDialog } from './ElicitationDialog';
import { ElicitationRequest } from '../api/types';

afterEach(() => {
  cleanup();
});

const elicitation: ElicitationRequest = {
  id: 'elicitation-1',
  message: 'Provide the missing configuration.',
  properties: {
    mode: {
      type: 'string',
      title: 'Mode',
      oneOf: [
        { const: 'fast', title: 'Fast' },
        { const: 'safe', title: 'Safe' },
      ],
    },
    note: {
      type: 'string',
      title: 'Note',
      description: 'Optional note for the agent',
    },
    confirmed: {
      type: 'boolean',
      title: 'Confirmed',
    },
    retries: {
      type: 'integer',
      title: 'Retries',
      default: 2,
    },
  },
  required: ['mode', 'note'],
};

describe('ElicitationDialog', () => {
  it('renders schema-driven controls and submits collected values', async () => {
    const onSubmit = vi.fn();
    const onDecline = vi.fn();
    const onCancel = vi.fn();
    const user = userEvent.setup();

    render(
      <ElicitationDialog
        elicitation={elicitation}
        onSubmit={onSubmit}
        onDecline={onDecline}
        onCancel={onCancel}
      />,
    );

    expect(screen.getByText('Provide the missing configuration.')).toBeTruthy();

    await user.selectOptions(screen.getByLabelText('Mode*'), 'safe');
    await user.type(screen.getByLabelText('Note*'), 'Use conservative defaults');
    await user.click(screen.getByLabelText('Confirmed'));

    const retriesInput = screen.getByLabelText('Retries') as HTMLInputElement;
    await user.clear(retriesInput);
    await user.type(retriesInput, '5');

    await user.click(screen.getByRole('button', { name: 'Submit' }));

    expect(onSubmit).toHaveBeenCalledWith({
      mode: 'safe',
      note: 'Use conservative defaults',
      confirmed: true,
      retries: 5,
    });
    expect(onDecline).not.toHaveBeenCalled();
    expect(onCancel).not.toHaveBeenCalled();
  });

  it('invokes decline and cancel actions', async () => {
    const onSubmit = vi.fn();
    const onDecline = vi.fn();
    const onCancel = vi.fn();
    const user = userEvent.setup();

    const { unmount } = render(
      <ElicitationDialog
        elicitation={elicitation}
        onSubmit={onSubmit}
        onDecline={onDecline}
        onCancel={onCancel}
      />,
    );

    await user.click(screen.getByRole('button', { name: 'Decline' }));
    expect(onDecline).toHaveBeenCalledTimes(1);
    expect(onCancel).not.toHaveBeenCalled();

    unmount();

    render(
      <ElicitationDialog
        elicitation={elicitation}
        onSubmit={onSubmit}
        onDecline={onDecline}
        onCancel={onCancel}
      />,
    );

    await user.click(screen.getByRole('button', { name: 'Cancel' }));

    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it('applies dialog accessibility semantics and closes on Escape', async () => {
    const onSubmit = vi.fn();
    const onDecline = vi.fn();
    const onCancel = vi.fn();
    const user = userEvent.setup();

    render(
      <ElicitationDialog
        elicitation={elicitation}
        onSubmit={onSubmit}
        onDecline={onDecline}
        onCancel={onCancel}
      />,
    );

    const dialog = screen.getByRole('dialog', { name: 'Agent Request' });
    expect(dialog.getAttribute('aria-modal')).toBe('true');
    expect(dialog.getAttribute('aria-labelledby')).toBeTruthy();

    await user.keyboard('{Escape}');
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('traps focus within dialog when tabbing', async () => {
    const onSubmit = vi.fn();
    const onDecline = vi.fn();
    const onCancel = vi.fn();
    const user = userEvent.setup();

    render(
      <ElicitationDialog
        elicitation={elicitation}
        onSubmit={onSubmit}
        onDecline={onDecline}
        onCancel={onCancel}
      />,
    );

    const dialog = screen.getByRole('dialog', { name: 'Agent Request' });
    const submitButton = within(dialog).getByRole('button', { name: 'Submit' });
    const closeButton = within(dialog).getByRole('button', { name: 'Close' });

    submitButton.focus();
    expect(document.activeElement).toBe(submitButton);

    await user.tab();
    expect(document.activeElement).toBe(closeButton);

    closeButton.focus();
    expect(document.activeElement).toBe(closeButton);

    await user.tab({ shift: true });
    expect(document.activeElement).toBe(submitButton);
  });

  it('closes when clicking the backdrop', async () => {
    const onSubmit = vi.fn();
    const onDecline = vi.fn();
    const onCancel = vi.fn();
    const user = userEvent.setup();

    const { container } = render(
      <ElicitationDialog
        elicitation={elicitation}
        onSubmit={onSubmit}
        onDecline={onDecline}
        onCancel={onCancel}
      />,
    );

    const backdrop = container.firstElementChild as HTMLElement;
    await user.click(backdrop);

    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(onSubmit).not.toHaveBeenCalled();
    expect(onDecline).not.toHaveBeenCalled();
  });
});
