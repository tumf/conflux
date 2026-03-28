/**
 * API Types matching Rust src/remote/types.rs and src/tui/types.rs
 */

export type ProjectStatus = 'idle' | 'running' | 'stopped';
export type ChangeStatus =
  | 'idle'
  | 'queued'
  | 'applying'
  | 'accepting'
  | 'archiving'
  | 'resolving'
  | 'archived'
  | 'merged'
  | 'error';

export interface RemoteProject {
  id: string;
  name: string;
  repo: string;
  branch: string;
  status: ProjectStatus;
  is_busy: boolean;
  error: string | null;
  changes: RemoteChange[];
}

export interface RemoteChange {
  id: string;
  /** Project identifier - matches `project` field from Rust RemoteChange */
  project: string;
  status: string;
  iteration_number: number | null;
  completed_tasks: number;
  total_tasks: number;
  last_modified: string;
}

export interface RemoteLogEntry {
  message: string;
  level: string;
  change_id: string | null;
  timestamp: string;
  project_id?: string | null;
  operation?: string | null;
  iteration?: number | null;
}

/** Merge conflict information for a worktree */
export interface MergeConflictInfo {
  conflict_files: string[];
}

/** Information about a git worktree, matching Rust WorktreeInfo */
export interface WorktreeInfo {
  /** Path to the worktree */
  path: string;
  /** Current HEAD commit (short hash or symbolic ref) */
  head: string;
  /** Branch name (empty if detached) */
  branch: string;
  /** Whether HEAD is detached */
  is_detached: boolean;
  /** Whether this is the main worktree */
  is_main: boolean;
  /** Merge conflict information (null if no conflicts) */
  merge_conflict: MergeConflictInfo | null;
  /** Whether this worktree has commits ahead of the base branch */
  has_commits_ahead: boolean;
  /** Whether a merge operation is in progress */
  is_merging: boolean;
}

export interface FullState {
  projects: RemoteProject[];
  worktrees?: Record<string, WorktreeInfo[]>;
}
