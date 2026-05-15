/**
 * @vitest-environment jsdom
 */

import React from 'react';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { WorktreesPanel } from './WorktreesPanel';
import { WorktreeInfo } from '../api/types';

function makeWorktree(branch: string, overrides: Partial<WorktreeInfo> = {}): WorktreeInfo {
  return {
    path: `/tmp/${branch}`,
    head: 'abc1234',
    branch,
    is_detached: false,
    is_main: false,
    merge_conflict: null,
    has_commits_ahead: false,
    is_merging: false,
    ...overrides,
  };
}

const noop = vi.fn();

afterEach(() => {
  cleanup();
  noop.mockClear();
});

describe('WorktreesPanel', () => {
  it('marks only the matching branch row as deleting', () => {
    render(
      <WorktreesPanel
        worktrees={[makeWorktree('feature-a'), makeWorktree('feature-b')]}
        selectedProjectId="project-1"
        onMerge={noop}
        onDelete={noop}
        onCreate={noop}
        onRefresh={noop}
        isLoading={false}
        deletingWorktreeBranch="feature-a"
      />,
    );

    expect(screen.getByLabelText('Deleting feature-a')).toBeTruthy();
    expect(screen.queryByLabelText('Deleting feature-b')).toBeNull();
    expect(screen.getByText('feature-b')).toBeTruthy();
  });
});
