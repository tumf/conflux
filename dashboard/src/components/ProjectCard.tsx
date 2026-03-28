/**
 * ProjectCard Component
 * Displays a single project with status and action buttons
 */

import React from 'react';
import { Play, Square, RefreshCw, Trash2 } from 'lucide-react';
import { RemoteProject } from '../api/types';

interface ProjectCardProps {
  project: RemoteProject;
  isSelected: boolean;
  onSelect: (projectId: string) => void;
  onRun: (projectId: string) => void;
  onStop: (projectId: string) => void;
  onGitSync: (projectId: string) => void;
  onDelete: (projectId: string) => void;
  isLoading: boolean;
}

const statusBadgeColors: Record<string, string> = {
  idle: 'bg-slate-600 text-white',
  running: 'bg-green-600 text-white',
  stopped: 'bg-red-600 text-white',
};

export function ProjectCard({
  project,
  isSelected,
  onSelect,
  onRun,
  onStop,
  onGitSync,
  onDelete,
  isLoading,
}: ProjectCardProps) {
  const projectName = `${project.repo}@${project.branch}`;

  return (
    <div
      onClick={() => onSelect(project.id)}
      className={`cursor-pointer rounded-lg border p-4 transition-all ${
        isSelected
          ? 'border-color-accent bg-color-surface'
          : 'border-color-border bg-color-surface-secondary hover:border-color-accent'
      }`}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          onSelect(project.id);
        }
      }}
    >
      <div className="mb-3 flex items-start justify-between">
        <div className="flex-1">
          <h3 className="text-lg font-semibold text-color-text">{projectName}</h3>
          {project.error && (
            <p className="mt-1 text-sm text-color-error">{project.error}</p>
          )}
        </div>
        <div className={`rounded px-2 py-1 text-xs font-semibold ${statusBadgeColors[project.status]}`}>
          {project.status}
        </div>
      </div>

      <div className="flex gap-2">
        <button
          onClick={(e) => {
            e.stopPropagation();
            onRun(project.id);
          }}
          disabled={isLoading || project.status === 'running'}
          className="flex-1 rounded bg-green-600 px-3 py-2 text-sm text-white hover:bg-green-700 disabled:opacity-50"
          aria-label={`Run project ${projectName}`}
        >
          <Play className="inline h-4 w-4" />
          <span className="ml-1">Run</span>
        </button>

        <button
          onClick={(e) => {
            e.stopPropagation();
            onStop(project.id);
          }}
          disabled={isLoading || project.status !== 'running'}
          className="flex-1 rounded bg-red-600 px-3 py-2 text-sm text-white hover:bg-red-700 disabled:opacity-50"
          aria-label={`Stop project ${projectName}`}
        >
          <Square className="inline h-4 w-4" />
          <span className="ml-1">Stop</span>
        </button>

        <button
          onClick={(e) => {
            e.stopPropagation();
            onGitSync(project.id);
          }}
          disabled={isLoading}
          className="flex-1 rounded bg-blue-600 px-3 py-2 text-sm text-white hover:bg-blue-700 disabled:opacity-50"
          aria-label={`Git sync project ${projectName}`}
        >
          <RefreshCw className="inline h-4 w-4" />
          <span className="ml-1">Sync</span>
        </button>

        <button
          onClick={(e) => {
            e.stopPropagation();
            onDelete(project.id);
          }}
          disabled={isLoading}
          className="rounded bg-gray-600 px-3 py-2 text-sm text-white hover:bg-gray-700 disabled:opacity-50"
          aria-label={`Delete project ${projectName}`}
        >
          <Trash2 className="h-4 w-4" />
        </button>
      </div>
    </div>
  );
}
