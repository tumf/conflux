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

/** An active command occupying a worktree root */
export interface ActiveCommand {
  project_id: string;
  root: string;
  operation: string;
  started_at: string;
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
  /** Currently active commands across all worktree roots */
  active_commands?: ActiveCommand[];
}

// ─── Overview Dashboard Types ─────────────────────────────────────────────────

export interface StatsOverview {
  summary: {
    success_count: number;
    failure_count: number;
    in_progress_count: number;
    /** Optional overall average duration in milliseconds */
    average_duration_ms?: number | null;
    /** Optional per-operation average duration in milliseconds */
    average_duration_by_operation?: Record<string, number>;
  };
  recent_events: ChangeEventSummary[];
  project_stats: ProjectStats[];
}

export interface ChangeEventSummary {
  project_id: string;
  project_name?: string;
  change_id: string;
  operation: string;
  result: 'success' | 'failure' | 'in_progress' | string;
  timestamp: string;
}

export interface ProjectStats {
  project_id: string;
  project_name?: string;
  apply_success_rate: number;
  average_duration_ms: number | null;
  success_count: number;
  failure_count: number;
  in_progress_count: number;
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

export type ProposalSessionStatus = 'active' | 'merging' | 'timed_out' | 'closed';

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
  id: string;
  title: string | null;
}

export type ProposalWsMessageType =
  | 'prompt'
  | 'elicitation_response'
  | 'cancel'
  | 'user_message'
  | 'agent_message_chunk'
  | 'tool_call'
  | 'tool_call_update'
  | 'elicitation'
  | 'turn_complete'
  | 'error';

/** Messages sent from client to server */
export type ProposalWsClientMessage =
  | { type: 'prompt'; content: string }
  | {
      type: 'elicitation_response';
      elicitation_id: string;
      action: 'accept' | 'decline' | 'cancel';
      data?: Record<string, unknown>;
    }
  | { type: 'cancel' };

/** Messages received from server */
export type ProposalWsServerMessage =
  | { type: 'user_message'; id: string; content: string; timestamp: string }
  | { type: 'agent_message_chunk'; text: string; message_id?: string; turn_id?: string }
  | {
      type: 'tool_call';
      tool_call_id: string;
      title: string;
      kind: string;
      status: ToolCallStatus;
      message_id?: string;
      turn_id?: string;
    }
  | {
      type: 'tool_call_update';
      tool_call_id: string;
      status: ToolCallStatus;
      content: unknown[];
      message_id?: string;
      turn_id?: string;
    }
  | {
      type: 'elicitation';
      request_id: string;
      mode: string;
      message: string;
      schema?: {
        type?: string;
        properties?: Record<string, ElicitationProperty>;
        required?: string[];
      } | null;
    }
  | { type: 'turn_complete'; stop_reason: string; message_id?: string; turn_id?: string }
  | { type: 'error'; message: string };

export type ProposalChatRole = 'user' | 'assistant';

export type ToolCallStatus = 'pending' | 'in_progress' | 'completed' | 'failed';

export interface ToolCallInfo {
  id: string;
  title: string;
  status: ToolCallStatus;
}

export type ProposalChatSendStatus = 'sent' | 'pending' | 'failed';

export interface ProposalChatMessage {
  id: string;
  role: ProposalChatRole;
  content: string;
  /** ISO 8601 timestamp */
  timestamp: string;
  /** User message delivery state for websocket retry UX */
  sendStatus?: ProposalChatSendStatus;
  /** Server turn identifier used for chunk/tool-call correlation */
  turn_id?: string;
  /** Message restored from backend history endpoint */
  hydrated?: boolean;
  /** Tool calls associated with this message (agent only) */
  tool_calls?: ToolCallInfo[];
}

export interface ProposalSessionMessageHistoryResponse {
  messages: ProposalChatMessage[];
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
