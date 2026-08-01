/**
 * @vitest-environment jsdom
 */

import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { ChangeRow } from './ChangeRow';
import { BLOCKER_KINDS, BlockerKind, RemoteChange, toBlockerKind } from '../api/types';

const toggleChangeSelectionMock = vi.fn().mockResolvedValue({ change_id: 'change-a', selected: false });
const stopAndDequeueChangeMock = vi.fn().mockResolvedValue(undefined);
const toastErrorMock = vi.fn();

vi.mock('../api/restClient', () => ({
  APIError: class APIError extends Error {
    constructor(public status: number, message: string) {
      super(message);
    }
  },
  toggleChangeSelection: (...args: unknown[]) => toggleChangeSelectionMock(...args),
  stopAndDequeueChange: (...args: unknown[]) => stopAndDequeueChangeMock(...args),
}));

vi.mock('sonner', () => ({
  toast: {
    error: (...args: unknown[]) => toastErrorMock(...args),
  },
}));

function makeChange(status: RemoteChange['status']): RemoteChange {
  return {
    id: 'change-a',
    project: 'project-1',
    completed_tasks: 1,
    total_tasks: 2,
    last_modified: '2026-01-01T00:00:00Z',
    status,
    iteration_number: null,
    selected: true,
  };
}

function makeBlockedChange(
  blocker_kind: BlockerKind,
  blocker_detail: string,
): RemoteChange {
  return { ...makeChange('blocked'), blocker_kind, blocker_detail };
}

afterEach(() => {
  toggleChangeSelectionMock.mockReset();
  toggleChangeSelectionMock.mockResolvedValue({ change_id: 'change-a', selected: false });
  stopAndDequeueChangeMock.mockClear();
  toastErrorMock.mockClear();
  cleanup();
});

describe('ChangeRow', () => {
  it('shows stop button for active changes, opens confirmation dialog, calls API on confirm', () => {
    render(<ChangeRow change={makeChange('applying')} />);

    const button = screen.getByRole('button', { name: 'Stop and dequeue change-a' });
    fireEvent.click(button);

    // API should NOT be called immediately
    expect(stopAndDequeueChangeMock).not.toHaveBeenCalled();

    // Confirmation dialog should be shown
    expect(screen.getByText('Force Kill Change')).toBeTruthy();

    // Click the confirm button
    const confirmButton = screen.getByRole('button', { name: 'Force Kill' });
    fireEvent.click(confirmButton);

    expect(stopAndDequeueChangeMock).toHaveBeenCalledWith('project-1', 'change-a');
  });

  it('closes confirmation dialog on cancel without calling API', () => {
    render(<ChangeRow change={makeChange('applying')} />);

    const button = screen.getByRole('button', { name: 'Stop and dequeue change-a' });
    fireEvent.click(button);

    // Click cancel
    const cancelButton = screen.getByRole('button', { name: 'Cancel' });
    fireEvent.click(cancelButton);

    expect(stopAndDequeueChangeMock).not.toHaveBeenCalled();
    expect(screen.queryByText('Force Kill Change')).toBeNull();
  });

  it('does not show stop-and-dequeue button for not queued change', () => {
    render(<ChangeRow change={makeChange('not queued')} />);

    expect(screen.queryByRole('button', { name: 'Stop and dequeue change-a' })).toBeNull();
  });

  it('does not show stop-and-dequeue button for rejected change', () => {
    render(<ChangeRow change={makeChange('rejected')} />);

    expect(screen.queryByRole('button', { name: 'Stop and dequeue change-a' })).toBeNull();
  });

  it('allows stalled changes to be selected again', () => {
    const onOptimisticSelectionChange = vi.fn();
    const change = { ...makeChange('stalled'), selected: false };

    render(
      <ChangeRow
        change={change}
        onOptimisticSelectionChange={onOptimisticSelectionChange}
      />,
    );

    fireEvent.click(screen.getByRole('checkbox', { name: 'Select change change-a' }));

    expect(onOptimisticSelectionChange).toHaveBeenCalledWith(change, true);
    expect(toggleChangeSelectionMock).toHaveBeenCalledWith('project-1', 'change-a');
  });

  it('applies optimistic selection immediately when toggled', () => {
    const onOptimisticSelectionChange = vi.fn();
    const change = { ...makeChange('error'), selected: false };

    render(
      <ChangeRow
        change={change}
        onOptimisticSelectionChange={onOptimisticSelectionChange}
      />,
    );

    fireEvent.click(screen.getByRole('checkbox', { name: 'Select change change-a' }));

    expect(onOptimisticSelectionChange).toHaveBeenCalledWith(change, true);
    expect(toggleChangeSelectionMock).toHaveBeenCalledWith('project-1', 'change-a');
  });

  it('rolls back optimistic selection and shows error toast on toggle failure', async () => {
    const failingChange = { ...makeChange('error'), selected: false };
    const onOptimisticSelectionChange = vi.fn();
    const onOptimisticSelectionRollback = vi.fn();

    toggleChangeSelectionMock.mockRejectedValueOnce(new Error('network down'));

    render(
      <ChangeRow
        change={failingChange}
        onOptimisticSelectionChange={onOptimisticSelectionChange}
        onOptimisticSelectionRollback={onOptimisticSelectionRollback}
      />,
    );

    fireEvent.click(screen.getByRole('checkbox', { name: 'Select change change-a' }));

    await waitFor(() => {
      expect(onOptimisticSelectionRollback).toHaveBeenCalledWith(failingChange);
    });

    expect(onOptimisticSelectionChange).toHaveBeenCalledWith(failingChange, true);
    expect(toastErrorMock).toHaveBeenCalled();
  });
  it('renders a dependency wait and an external wait as blocked while keeping their kinds distinct', () => {
    const { unmount } = render(
      <ChangeRow change={makeBlockedChange('dependency', 'waiting on unarchived dependency alpha')} />,
    );
    expect(screen.getByText('blocked:dependency')).toBeTruthy();
    expect(
      screen.getByTestId('blocker-detail').textContent,
    ).toContain('waiting on unarchived dependency alpha');
    unmount();

    render(
      <ChangeRow
        change={makeBlockedChange(
          'external',
          'external blocker (credential) reported by acceptance: STAGING_API_KEY is unset; unblock when the key is present; next action provision it',
        )}
      />,
    );
    const badge = screen.getByText('blocked:external');
    expect(badge.getAttribute('data-blocker-kind')).toBe('external');
    const detail = screen.getByTestId('blocker-detail').textContent ?? '';
    expect(detail).toContain('credential');
    expect(detail).toContain('unblock when');
    expect(detail).toContain('next action');
  });

  it('keeps stalled rows stalled, with detail but without a blocker kind', () => {
    render(
      <ChangeRow
        change={{
          ...makeChange('stalled'),
          blocker_detail: 'acceptance stopped after repeated findings',
        }}
      />,
    );

    expect(screen.getByText('stalled')).toBeTruthy();
    expect(screen.queryByText('stalled:external')).toBeNull();
    expect(
      screen.getByTestId('blocker-detail').textContent,
    ).toContain('repeated findings');
  });

  it('renders a blocked row without a reported kind as plain blocked', () => {
    render(<ChangeRow change={makeChange('blocked')} />);

    expect(screen.getByText('blocked')).toBeTruthy();
    expect(screen.queryByTestId('blocker-detail')).toBeNull();
  });

  // `blocker_kind` arrives as untrusted JSON, so the closed set is enforced at
  // runtime rather than by a type the build erases.
  it('rejects a blocker kind outside the closed set', () => {
    expect(BLOCKER_KINDS).toEqual(['dependency', 'external']);
    expect(toBlockerKind('external')).toBe('external');
    expect(toBlockerKind('dependency')).toBe('dependency');
    for (const invalid of ['flaky', '', null, undefined, 0, {}]) {
      expect(toBlockerKind(invalid)).toBeNull();
    }

    const unknownKind = {
      ...makeChange('blocked'),
      blocker_kind: 'flaky',
    } as unknown as RemoteChange;

    render(<ChangeRow change={unknownKind} />);

    // The row stays plainly `blocked`: the dashboard never renders a kind it
    // cannot explain.
    expect(screen.getByText('blocked')).toBeTruthy();
    expect(screen.queryByText('blocked:flaky')).toBeNull();
  });
});
