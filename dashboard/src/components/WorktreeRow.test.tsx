/**
 * @vitest-environment jsdom
 */

import React from 'react';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { WorktreeRow } from './WorktreeRow';
import { WorktreeInfo } from '../api/types';

function makeWorktree(overrides: Partial<WorktreeInfo> = {}): WorktreeInfo {
  return {
    path: '/tmp/feature-a',
    head: 'abc1234',
    branch: 'feature-a',
    is_detached: false,
    is_main: false,
    merge_conflict: null,
    has_commits_ahead: true,
    is_merging: false,
    ...overrides,
  };
}

afterEach(() => {
  cleanup();
});

describe('WorktreeRow', () => {
  it('shows deleting UI and disables merge/delete controls when deleting', () => {
    const onMerge = vi.fn();
    const onDelete = vi.fn();

    render(
      <WorktreeRow
        worktree={makeWorktree()}
        onMerge={onMerge}
        onDelete={onDelete}
        isDeleting
      />,
    );

    expect(screen.getByText('Deleting...')).toBeTruthy();
    expect(screen.getByLabelText('Deleting feature-a')).toBeTruthy();
    const deletingControls = screen.getAllByTitle('Deleting worktree');
    expect(deletingControls).toHaveLength(2);
    deletingControls.forEach((control) => {
      expect(control).toHaveProperty('disabled', true);
    });
  });

  it('suppresses row selection while deleting', () => {
    const onClickWorktree = vi.fn();

    render(
      <WorktreeRow
        worktree={makeWorktree()}
        onClickWorktree={onClickWorktree}
        isDeleting
      />,
    );

    fireEvent.click(screen.getByText('feature-a'));

    expect(onClickWorktree).not.toHaveBeenCalled();
  });

  it('allows row selection and actions when not deleting', () => {
    const onClickWorktree = vi.fn();
    const onMerge = vi.fn();
    const onDelete = vi.fn();

    render(
      <WorktreeRow
        worktree={makeWorktree()}
        onClickWorktree={onClickWorktree}
        onMerge={onMerge}
        onDelete={onDelete}
      />,
    );

    fireEvent.click(screen.getByText('feature-a'));
    fireEvent.click(screen.getByTitle('Merge branch'));
    fireEvent.click(screen.getByTitle('Delete worktree'));

    expect(onClickWorktree).toHaveBeenCalledWith('feature-a');
    expect(onMerge).toHaveBeenCalledWith('feature-a');
    expect(onDelete).toHaveBeenCalledWith('feature-a');
  });
});
