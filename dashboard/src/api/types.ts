/**
 * API Types matching Rust src/remote/types.rs and src/tui/types.rs
 */

export type ProjectStatus = 'idle' | 'running' | 'stopped';
export type OrchestrationStatus = 'idle' | 'running' | 'stopped';
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
  /** Display name in "repo@branch" format */
  name: string;
  repo: string;
  branch: string;
  status: ProjectStatus;
  is_busy: boolean;
  error: string | null;
  /** Changes nested within this project (from server) */
  changes: RemoteChange[];
}

export interface RemoteChange {
  id: string;
  /** Project identifier this change belongs to */
  project: string;
  completed_tasks: number;
  total_tasks: number;
  /** ISO 8601 timestamp of last modification */
  last_modified: string;
  status: ChangeStatus;
  iteration_number: number | null;
  /** Whether this change is selected for execution */
  selected: boolean;
}

export interface RemoteLogEntry {
  message: string;
  level: 'info' | 'warn' | 'error' | 'success';
  change_id: string | null;
  /** ISO 8601 timestamp */
  timestamp: string;
  project_id: string | null;
  operation: string | null;
  iteration: number | null;
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
  /** Flattened changes extracted from projects for easy access */
  changes: RemoteChange[];
  worktrees?: Record<string, WorktreeInfo[]>;
  /** Whether git/sync is available (resolve_command is configured on server) */
  sync_available?: boolean;
  /** Global orchestration status */
  orchestration_status?: OrchestrationStatus;
}

// ─── File Viewer Types ───────────────────────────────────────────────────────

/** A single entry in the file tree returned by the server */
export interface FileTreeEntry {
  name: string;
  path: string;
  type: 'file' | 'directory';
  children?: FileTreeEntry[];
}

/** Response from the file content API */
export interface FileContentResponse {
  path: string;
  content: string | null;
  size: number;
  truncated: boolean;
  binary: boolean;
}

/** Context for the file browser: which root to browse */
export interface FileBrowseContext {
  type: 'change' | 'worktree';
  changeId?: string;
  worktreeBranch?: string;
}

// ─── Proposal Session Types ──────────────────────────────────────────────────

export type ProposalSessionStatus = 'active' | 'merging' | 'closed';

export interface ProposalSession {
  id: string;
  project_id: string;
  status: ProposalSessionStatus;
  /** Worktree branch name backing this session */
  worktree_branch: string;
  /** Whether the worktree has uncommitted changes */
  is_dirty: boolean;
  /** List of uncommitted file paths (populated when dirty) */
  uncommitted_files: string[];
  /** ISO 8601 timestamp */
  created_at: string;
  /** ISO 8601 timestamp */
  updated_at: string;
}

export interface ProposalSessionChange {
  change_id: string;
  title: string;
}

export type ProposalChatRole = 'user' | 'assistant';

export type ToolCallStatus = 'pending' | 'in_progress' | 'completed' | 'failed';

export interface ToolCallInfo {
  id: string;
  title: string;
  status: ToolCallStatus;
}

export interface ProposalChatMessage {
  id: string;
  role: ProposalChatRole;
  content: string;
  /** ISO 8601 timestamp */
  timestamp: string;
  /** Tool calls associated with this message (agent only) */
  tool_calls?: ToolCallInfo[];
}

/** JSON Schema property for elicitation forms */
export interface ElicitationProperty {
  type: 'string' | 'boolean' | 'number' | 'integer';
  title?: string;
  description?: string;
  /** oneOf or enum values for select/radio */
  oneOf?: Array<{ const: string; title: string }>;
  enum?: string[];
  default?: unknown;
}

export interface ElicitationRequest {
  id: string;
  /** Human-readable message describing what is needed */
  message: string;
  /** JSON Schema properties for form fields */
  properties: Record<string, ElicitationProperty>;
  /** Required field names */
  required?: string[];
}

// ─── Proposal WebSocket Message Types ────────────────────────────────────────

export type ProposalWsMessageType =
  | 'prompt'
  | 'elicitation_response'
  | 'cancel'
  | 'assistant_chunk'
  | 'assistant_message'
  | 'tool_call_start'
  | 'tool_call_update'
  | 'elicitation_request'
  | 'session_update'
  | 'error';

/** Messages sent from client to server */
export type ProposalWsClientMessage =
  | { type: 'prompt'; content: string }
  | { type: 'elicitation_response'; elicitation_id: string; action: 'accept' | 'decline' | 'cancel'; data?: Record<string, unknown> }
  | { type: 'cancel' };

/** Messages received from server */
export type ProposalWsServerMessage =
  | { type: 'assistant_chunk'; content: string; message_id: string }
  | { type: 'assistant_message'; message: ProposalChatMessage }
  | { type: 'tool_call_start'; tool_call: ToolCallInfo; message_id: string }
  | { type: 'tool_call_update'; tool_call_id: string; status: ToolCallStatus; message_id: string }
  | { type: 'elicitation_request'; elicitation: ElicitationRequest }
  | { type: 'session_update'; session: ProposalSession }
  | { type: 'error'; message: string };
