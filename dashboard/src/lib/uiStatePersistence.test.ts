/**
 * @vitest-environment jsdom
 */

import { describe, expect, it } from 'vitest';

import { RemoteProject, WorktreeInfo } from '../api/types';
import {
  parseFileBrowseContext,
  parsePersistedTabState,
  resolvePersistedBrowseSelection,
  serializeFileBrowseContext,
  UI_STATE_KEYS,
} from './uiStatePersistence';

const createProject = (id: string, changeIds: string[] = []): RemoteProject => ({
  id,
  name: `${id}@main`,
  repo: id,
  branch: 'main',
  status: 'idle',
  is_busy: false,
  error: null,
  changes: changeIds.map((changeId) => ({
    id: changeId,
    project: id,
    completed_tasks: 0,
    total_tasks: 0,
    last_modified: '2026-04-01T00:00:00.000Z',
    status: 'idle',
    iteration_number: null,
    selected: false,
  })),
});

const createWorktree = (branch: string): WorktreeInfo => ({
  path: `/tmp/${branch}`,
  head: 'abc1234',
  branch,
  is_detached: false,
  is_main: false,
  merge_conflict: null,
  has_commits_ahead: false,
  is_merging: false,
});

describe('uiStatePersistence', () => {
  it('serializes and parses valid file browse contexts', () => {
    const changeContext = { type: 'change' as const, changeId: 'change-1' };
    const worktreeContext = { type: 'worktree' as const, worktreeBranch: 'feature-x' };

    expect(parseFileBrowseContext(serializeFileBrowseContext(changeContext))).toEqual(changeContext);
    expect(parseFileBrowseContext(serializeFileBrowseContext(worktreeContext))).toEqual(worktreeContext);
  });

  it('returns null when file browse context payload is invalid', () => {
    expect(parseFileBrowseContext('not-json')).toBeNull();
    expect(parseFileBrowseContext(JSON.stringify({ type: 'change' }))).toBeNull();
    expect(parseFileBrowseContext(JSON.stringify({ type: 'worktree' }))).toBeNull();
  });

  it('parses only valid persisted tab keys', () => {
    const tabs = parsePersistedTabState({
      [UI_STATE_KEYS.desktopCenterTab]: 'worktrees',
      [UI_STATE_KEYS.desktopRightTab]: 'files',
      [UI_STATE_KEYS.mobileActiveTab]: 'changes',
      ignored: 'value',
    });

    expect(tabs).toEqual({
      desktopCenterTab: 'worktrees',
      desktopRightTab: 'files',
      mobileActiveTab: 'changes',
    });

    expect(
      parsePersistedTabState({
        [UI_STATE_KEYS.desktopCenterTab]: 'invalid',
        [UI_STATE_KEYS.desktopRightTab]: 'invalid',
        [UI_STATE_KEYS.mobileActiveTab]: 'invalid',
      }),
    ).toEqual({});
  });

  it('restores persisted change context when target still exists', () => {
    const result = resolvePersistedBrowseSelection({
      uiState: {
        [UI_STATE_KEYS.fileBrowseContext]: serializeFileBrowseContext({
          type: 'change',
          changeId: 'change-a',
        }),
      },
      selectedProjectId: 'project-1',
      projects: [createProject('project-1', ['change-a'])],
      worktreesByProjectId: {},
    });

    expect(result).toEqual({
      status: 'restored',
      context: { type: 'change', changeId: 'change-a' },
      tabs: {
        desktopCenterTab: 'changes',
        desktopRightTab: 'files',
        mobileActiveTab: 'files',
      },
    });
  });

  it('restores persisted worktree context when worktree still exists', () => {
    const result = resolvePersistedBrowseSelection({
      uiState: {
        [UI_STATE_KEYS.fileBrowseContext]: serializeFileBrowseContext({
          type: 'worktree',
          worktreeBranch: 'feature-x',
        }),
      },
      selectedProjectId: 'project-1',
      projects: [createProject('project-1', [])],
      worktreesByProjectId: { 'project-1': [createWorktree('feature-x')] },
    });

    expect(result).toEqual({
      status: 'restored',
      context: { type: 'worktree', worktreeBranch: 'feature-x' },
      tabs: {
        desktopCenterTab: 'worktrees',
        desktopRightTab: 'files',
        mobileActiveTab: 'files',
      },
    });
  });

  it('defers worktree restore until worktree list is hydrated', () => {
    const result = resolvePersistedBrowseSelection({
      uiState: {
        [UI_STATE_KEYS.fileBrowseContext]: serializeFileBrowseContext({
          type: 'worktree',
          worktreeBranch: 'feature-x',
        }),
      },
      selectedProjectId: 'project-1',
      projects: [createProject('project-1', [])],
      worktreesByProjectId: undefined,
    });

    expect(result).toEqual({ status: 'defer' });
  });

  it('marks stale browse context when persisted target is missing', () => {
    const result = resolvePersistedBrowseSelection({
      uiState: {
        [UI_STATE_KEYS.fileBrowseContext]: serializeFileBrowseContext({
          type: 'worktree',
          worktreeBranch: 'missing-branch',
        }),
      },
      selectedProjectId: 'project-1',
      projects: [createProject('project-1', [])],
      worktreesByProjectId: { 'project-1': [createWorktree('feature-x')] },
    });

    expect(result).toEqual({
      status: 'stale',
      keysToClear: [
        UI_STATE_KEYS.fileBrowseContext,
        UI_STATE_KEYS.desktopCenterTab,
        UI_STATE_KEYS.desktopRightTab,
        UI_STATE_KEYS.mobileActiveTab,
      ],
    });
  });
});
