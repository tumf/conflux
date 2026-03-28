/**
 * REST API Client for Conflux Server
 */

import { RemoteProject, WorktreeInfo } from './types';

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
  return fetchAPI(`/projects/${projectId}/worktrees/create`, {
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
  return fetchAPI(`/projects/${projectId}/worktrees/delete`, {
    method: 'POST',
    body: JSON.stringify({ branch_name: branchName }),
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
