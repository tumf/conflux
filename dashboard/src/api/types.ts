/**
 * API Types matching Rust src/remote/types.rs
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
  repo: string;
  branch: string;
  status: ProjectStatus;
  is_busy: boolean;
  error: string | null;
}

export interface RemoteChange {
  id: string;
  project_id: string;
  title: string;
  status: ChangeStatus;
  iteration_number: number;
  completed_tasks: number;
  total_tasks: number;
  error: string | null;
}

export interface RemoteLogEntry {
  project_id: string;
  timestamp: number; // Unix epoch ms
  level: 'info' | 'warn' | 'error';
  message: string;
}

export interface RemoteStateUpdate {
  projects: RemoteProject[];
  changes: RemoteChange[];
}

export interface FullState {
  projects: RemoteProject[];
  changes: RemoteChange[];
}
