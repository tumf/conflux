/**
 * @vitest-environment jsdom
 */

import React from 'react';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { ChangeRow } from './ChangeRow';
import { RemoteChange } from '../api/types';

const stopAndDequeueChangeMock = vi.fn().mockResolvedValue(undefined);

vi.mock('../api/restClient', () => ({
  toggleChangeSelection: vi.fn(),
  stopAndDequeueChange: (...args: unknown[]) => stopAndDequeueChangeMock(...args),
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

afterEach(() => {
  stopAndDequeueChangeMock.mockClear();
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
});
