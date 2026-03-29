/**
 * REST API Client for Conflux Server
 */

import {
  RemoteProject,
  WorktreeInfo,
  FileTreeEntry,
  FileContentResponse,
  ProposalSession,
  ProposalSessionChange,
} from './types';

const API_BASE = '/api/v1';

export class APIError extends Error {
  constructor(
    public status: number,
    public message: string,
  ) {
    super(message);
    this.name = 'APIError';
  }
}

async function fetchAPI<T>(
  endpoint: string,
  options: RequestInit = {},
): Promise<T> {
  const url = `${API_BASE}${endpoint}`;
  const response = await fetch(url, {
    headers: {
      'Content-Type': 'application/json',
      ...options.headers,
    },
    ...options,
  });

  if (!response.ok) {
    const text = await response.text();
    throw new APIError(response.status, text || response.statusText);
  }

  if (response.status === 204) {
    return undefined as T;
  }

  return response.json();
}

/**
 * Fetch backend version
 */
export async function fetchVersion(): Promise<{ version: string }> {
  return fetchAPI('/version', { method: 'GET' });
}

/**
 * Fetch current state: projects and changes
 */
export async function fetchProjectsState(): Promise<{
  projects: RemoteProject[];
}> {
  return fetchAPI('/projects/state', { method: 'GET' });
}

/**
 * Start global orchestration (run all projects with selected changes)
 */
export async function controlRun(): Promise<void> {
  return fetchAPI('/control/run', { method: 'POST' });
}

/**
 * Stop global orchestration (stop all running projects)
 */
export async function controlStop(): Promise<void> {
  return fetchAPI('/control/stop', { method: 'POST' });
}

/**
 * Git sync (pull + push) a project
 */
export async function gitSync(projectId: string): Promise<void> {
  return fetchAPI(`/projects/${projectId}/git/sync`, { method: 'POST' });
}

/**
 * Delete a project
 */
export async function deleteProject(projectId: string): Promise<void> {
  return fetchAPI(`/projects/${projectId}`, { method: 'DELETE' });
}

/**
 * Add a new project
 */
export async function addProject(remoteUrl: string, branch: string): Promise<void> {
  return fetchAPI('/projects', {
    method: 'POST',
    body: JSON.stringify({ remote_url: remoteUrl, branch }),
  });
}

/**
 * List worktrees for a project
 */
export async function listWorktrees(projectId: string): Promise<WorktreeInfo[]> {
  return fetchAPI(`/projects/${projectId}/worktrees`, { method: 'GET' });
}

/**
 * Create a new worktree for a project
 */
export async function createWorktree(
  projectId: string,
  changeId: string,
): Promise<WorktreeInfo> {
  return fetchAPI(`/projects/${projectId}/worktrees`, {
    method: 'POST',
    body: JSON.stringify({ change_id: changeId }),
  });
}

/**
 * Delete a worktree by branch name
 */
export async function deleteWorktree(
  projectId: string,
  branchName: string,
): Promise<void> {
  return fetchAPI(`/projects/${projectId}/worktrees/${encodeURIComponent(branchName)}`, {
    method: 'DELETE',
  });
}

/**
 * Merge a worktree branch into the base branch
 */
export async function mergeWorktree(
  projectId: string,
  branchName: string,
): Promise<void> {
  return fetchAPI(`/projects/${projectId}/worktrees/merge`, {
    method: 'POST',
    body: JSON.stringify({ branch_name: branchName }),
  });
}

/**
 * Refresh worktrees (re-scan from git)
 */
export async function refreshWorktrees(projectId: string): Promise<WorktreeInfo[]> {
  return fetchAPI(`/projects/${projectId}/worktrees/refresh`, {
    method: 'POST',
  });
}

/**
 * Toggle the selected state of a single change
 */
export async function toggleChangeSelection(
  projectId: string,
  changeId: string,
): Promise<{ change_id: string; selected: boolean }> {
  return fetchAPI(`/projects/${projectId}/changes/${changeId}/toggle`, {
    method: 'POST',
  });
}

/**
 * Toggle all changes for a project (select all / deselect all)
 */
export async function toggleAllChangeSelection(
  projectId: string,
): Promise<{ selected: boolean; count: number }> {
  return fetchAPI(`/projects/${projectId}/changes/toggle-all`, {
    method: 'POST',
  });
}

/**
 * Fetch file tree for a project
 */
export async function fetchFileTree(
  projectId: string,
  root: string = 'base',
): Promise<FileTreeEntry[]> {
  return fetchAPI(`/projects/${projectId}/files/tree?root=${encodeURIComponent(root)}`, {
    method: 'GET',
  });
}

/**
 * Fetch file content for a project
 */
export async function fetchFileContent(
  projectId: string,
  root: string,
  path: string,
): Promise<FileContentResponse> {
  return fetchAPI(
    `/projects/${projectId}/files/content?root=${encodeURIComponent(root)}&path=${encodeURIComponent(path)}`,
    { method: 'GET' },
  );
}

// ─── Terminal Session API ────────────────────────────────────────────────────

export interface TerminalSessionInfo {
  id: string;
  cwd: string;
  rows: number;
  cols: number;
  created_at: string;
  project_id: string;
  root: string;
}

export interface CreateTerminalRequest {
  project_id: string;
  root: string;
  rows?: number;
  cols?: number;
}

/**
 * Create a new terminal session
 */
export async function createTerminalSession(
  request: CreateTerminalRequest,
): Promise<TerminalSessionInfo> {
  return fetchAPI('/terminal/sessions', {
    method: 'POST',
    body: JSON.stringify(request),
  });
}

/**
 * List all terminal sessions
 */
export async function listTerminalSessions(): Promise<TerminalSessionInfo[]> {
  return fetchAPI('/terminal/sessions', { method: 'GET' });
}

/**
 * Delete a terminal session
 */
export async function deleteTerminalSession(sessionId: string): Promise<void> {
  return fetchAPI(`/terminal/sessions/${sessionId}`, { method: 'DELETE' });
}

/**
 * Resize a terminal session
 */
export async function resizeTerminalSession(
  sessionId: string,
  rows: number,
  cols: number,
): Promise<void> {
  return fetchAPI(`/terminal/sessions/${sessionId}/resize`, {
    method: 'POST',
    body: JSON.stringify({ rows, cols }),
  });
}

/**
 * Get the WebSocket URL for a terminal session
 */
export function getTerminalWsUrl(sessionId: string): string {
  const protocol = window.location.protocol === 'https:' ? 'wss' : 'ws';
  return `${protocol}://${window.location.host}/api/v1/terminal/sessions/${sessionId}/ws`;
}

// ─── Proposal Session API ────────────────────────────────────────────────────

/**
 * Create a new proposal session for a project
 */
export async function createProposalSession(
  projectId: string,
): Promise<ProposalSession> {
  return fetchAPI(`/projects/${projectId}/proposal-sessions`, {
    method: 'POST',
  });
}

/**
 * List all proposal sessions for a project
 */
export async function listProposalSessions(
  projectId: string,
): Promise<ProposalSession[]> {
  return fetchAPI(`/projects/${projectId}/proposal-sessions`, {
    method: 'GET',
  });
}

/**
 * Delete/close a proposal session
 */
export async function deleteProposalSession(
  projectId: string,
  sessionId: string,
  force: boolean = false,
): Promise<void> {
  return fetchAPI(
    `/projects/${projectId}/proposal-sessions/${sessionId}`,
    {
      method: 'DELETE',
      body: JSON.stringify({ force }),
    },
  );
}

/**
 * Merge a proposal session's worktree into the base branch
 */
export async function mergeProposalSession(
  projectId: string,
  sessionId: string,
): Promise<void> {
  return fetchAPI(
    `/projects/${projectId}/proposal-sessions/${sessionId}/merge`,
    { method: 'POST' },
  );
}

/**
 * List detected OpenSpec changes in a proposal session's worktree
 */
export async function listProposalSessionChanges(
  projectId: string,
  sessionId: string,
): Promise<ProposalSessionChange[]> {
  return fetchAPI(
    `/projects/${projectId}/proposal-sessions/${sessionId}/changes`,
    { method: 'GET' },
  );
}

/**
 * Get the WebSocket URL for a proposal session
 */
export function getProposalSessionWsUrl(
  projectId: string,
  sessionId: string,
): string {
  const protocol = window.location.protocol === 'https:' ? 'wss' : 'ws';
  return `${protocol}://${window.location.host}/api/v1/projects/${projectId}/proposal-sessions/${sessionId}/ws`;
}
