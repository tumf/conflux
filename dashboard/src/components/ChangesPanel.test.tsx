/**
 * @vitest-environment jsdom
 */

import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { ChangesPanel } from './ChangesPanel';
import { RemoteProject } from '../api/types';

const toggleAllChangeSelectionMock = vi.fn().mockResolvedValue({ selected: true, count: 2 });
const toastErrorMock = vi.fn();

vi.mock('../api/restClient', () => ({
  APIError: class APIError extends Error {
    constructor(public status: number, message: string) {
      super(message);
    }
  },
  toggleAllChangeSelection: (...args: unknown[]) => toggleAllChangeSelectionMock(...args),
}));

vi.mock('sonner', () => ({
  toast: {
    error: (...args: unknown[]) => toastErrorMock(...args),
  },
}));

function buildProject(): RemoteProject {
  return {
    id: 'project-1',
    name: 'project-1@main',
    repo: 'project-1',
    branch: 'main',
    status: 'idle',
    is_busy: false,
    error: null,
    sync_state: 'up_to_date',
    ahead_count: 0,
    behind_count: 0,
    sync_required: false,
    local_sha: null,
    remote_sha: null,
    last_remote_check_at: null,
    remote_check_error: null,
    changes: [
      {
        id: 'change-a',
        project: 'project-1',
        completed_tasks: 0,
        total_tasks: 2,
        last_modified: '2026-01-01T00:00:00Z',
        status: 'error',
        iteration_number: null,
        selected: false,
      },
      {
        id: 'change-b',
        project: 'project-1',
        completed_tasks: 0,
        total_tasks: 2,
        last_modified: '2026-01-01T00:00:00Z',
        status: 'not queued',
        iteration_number: null,
        selected: false,
      },
      {
        id: 'change-c',
        project: 'project-1',
        completed_tasks: 0,
        total_tasks: 2,
        last_modified: '2026-01-01T00:00:00Z',
        status: 'rejected',
        iteration_number: null,
        selected: false,
      },
    ],
  };
}

afterEach(() => {
  toggleAllChangeSelectionMock.mockReset();
  toggleAllChangeSelectionMock.mockResolvedValue({ selected: true, count: 2 });
  toastErrorMock.mockReset();
  cleanup();
});

describe('ChangesPanel bulk toggle', () => {
  it('applies optimistic selection to all non-rejected rows immediately', () => {
    const onOptimisticSelectionChange = vi.fn();

    render(
      <ChangesPanel
        projects={[buildProject()]}
        selectedProjectId="project-1"
        onOptimisticSelectionChange={onOptimisticSelectionChange}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Select all changes' }));

    expect(onOptimisticSelectionChange).toHaveBeenCalledTimes(2);
    expect(onOptimisticSelectionChange).toHaveBeenCalledWith(
      expect.objectContaining({ id: 'change-a' }),
      true,
    );
    expect(onOptimisticSelectionChange).toHaveBeenCalledWith(
      expect.objectContaining({ id: 'change-b' }),
      true,
    );
    expect(toggleAllChangeSelectionMock).toHaveBeenCalledWith('project-1');
  });

  it('rolls back optimistic selection when bulk toggle API fails', async () => {
    const onOptimisticSelectionChange = vi.fn();
    const onOptimisticSelectionRollback = vi.fn();
    toggleAllChangeSelectionMock.mockRejectedValueOnce(new Error('bulk failed'));

    render(
      <ChangesPanel
        projects={[buildProject()]}
        selectedProjectId="project-1"
        onOptimisticSelectionChange={onOptimisticSelectionChange}
        onOptimisticSelectionRollback={onOptimisticSelectionRollback}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Select all changes' }));

    await waitFor(() => {
      expect(onOptimisticSelectionRollback).toHaveBeenCalledTimes(2);
    });
    expect(toastErrorMock).toHaveBeenCalled();
  });
});
