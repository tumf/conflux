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
  it('shows stop-and-dequeue button for active changes and calls API', () => {
    render(<ChangeRow change={makeChange('applying')} />);

    const button = screen.getByRole('button', { name: 'Stop and dequeue change-a' });
    fireEvent.click(button);

    expect(stopAndDequeueChangeMock).toHaveBeenCalledWith('project-1', 'change-a');
  });

  it('does not show stop-and-dequeue button for not queued change', () => {
    render(<ChangeRow change={makeChange('not queued')} />);

    expect(screen.queryByRole('button', { name: 'Stop and dequeue change-a' })).toBeNull();
  });
});
