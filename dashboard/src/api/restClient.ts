/**
 * REST API Client for Conflux Server
 */

import { RemoteProject } from './types';

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
 * Fetch current state: projects and changes
 */
export async function fetchProjectsState(): Promise<{
  projects: RemoteProject[];
}> {
  return fetchAPI('/state', { method: 'GET' });
}

/**
 * Run a project
 */
export async function controlRun(projectId: string): Promise<void> {
  return fetchAPI(`/projects/${projectId}/run`, { method: 'POST' });
}

/**
 * Stop a project
 */
export async function controlStop(projectId: string): Promise<void> {
  return fetchAPI(`/projects/${projectId}/stop`, { method: 'POST' });
}

/**
 * Git sync (pull + push) a project
 */
export async function gitSync(projectId: string): Promise<void> {
  return fetchAPI(`/projects/${projectId}/git-sync`, { method: 'POST' });
}

/**
 * Delete a project
 */
export async function deleteProject(projectId: string): Promise<void> {
  return fetchAPI(`/projects/${projectId}`, { method: 'DELETE' });
}
