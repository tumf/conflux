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
  /** Whether git/sync is available (resolve_command configured on server) */
  syncAvailable: boolean;
}

const statusConfig: Record<string, { dot: string; text: string; bg: string }> = {
  idle: { dot: 'bg-[#52525b]', text: 'text-[#71717a]', bg: 'bg-[#18181b]' },
  running: { dot: 'bg-[#22c55e] animate-pulse', text: 'text-[#22c55e]', bg: 'bg-[#052e16]/40' },
  stopped: { dot: 'bg-[#ef4444]', text: 'text-[#ef4444]', bg: 'bg-[#450a0a]/40' },
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
  syncAvailable,
}: ProjectCardProps) {
  const [repo, branch] = [project.repo, project.branch];
  const cfg = statusConfig[project.status] ?? statusConfig.idle;

  return (
    <div
      onClick={() => onSelect(project.id)}
      className={`group cursor-pointer rounded-lg border p-3.5 transition-all ${
        isSelected
          ? 'border-[#6366f1] bg-[#1e1b4b]/30'
          : 'border-[#27272a] bg-[#111113] hover:border-[#3f3f46]'
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
      <div className="mb-3 flex items-start justify-between gap-2">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-1.5 truncate">
            <span className="truncate text-sm font-medium text-[#fafafa]">{repo}</span>
            <span className="text-[#52525b]">/</span>
            <span className="truncate text-sm text-[#a1a1aa]">{branch}</span>
          </div>
          {project.error && (
            <p className="mt-1 truncate text-xs text-[#ef4444]">{project.error}</p>
          )}
        </div>
        <div className={`flex shrink-0 items-center gap-1.5 rounded-md px-2 py-0.5 ${cfg.bg}`}>
          <div className={`size-1.5 rounded-full ${cfg.dot}`} />
          <span className={`text-xs font-medium ${cfg.text}`}>{project.status}</span>
        </div>
      </div>

      <div className="flex gap-1.5">
        <button
          onClick={(e) => { e.stopPropagation(); onRun(project.id); }}
          disabled={isLoading || project.status === 'running'}
          className="flex flex-1 items-center justify-center gap-1 rounded-md bg-[#166534]/60 px-2 py-1.5 text-xs font-medium text-[#22c55e] transition-colors hover:bg-[#166534]/80 disabled:cursor-not-allowed disabled:opacity-40"
          aria-label={`Run ${repo}@${branch}`}
        >
          <Play className="size-3" />
          Run
        </button>

        <button
          onClick={(e) => { e.stopPropagation(); onStop(project.id); }}
          disabled={isLoading || project.status !== 'running'}
          className="flex flex-1 items-center justify-center gap-1 rounded-md bg-[#450a0a]/60 px-2 py-1.5 text-xs font-medium text-[#ef4444] transition-colors hover:bg-[#450a0a]/80 disabled:cursor-not-allowed disabled:opacity-40"
          aria-label={`Stop ${repo}@${branch}`}
        >
          <Square className="size-3" />
          Stop
        </button>

        <button
          onClick={(e) => { e.stopPropagation(); onGitSync(project.id); }}
          disabled={isLoading || !syncAvailable}
          className="flex flex-1 items-center justify-center gap-1 rounded-md bg-[#1e3a5f]/60 px-2 py-1.5 text-xs font-medium text-[#3b82f6] transition-colors hover:bg-[#1e3a5f]/80 disabled:cursor-not-allowed disabled:opacity-40"
          aria-label={`Sync ${repo}@${branch}`}
          title={!syncAvailable ? 'resolve_command is not configured' : undefined}
        >
          <RefreshCw className="size-3" />
          Sync
        </button>

        <button
          onClick={(e) => { e.stopPropagation(); onDelete(project.id); }}
          disabled={isLoading}
          className="flex items-center justify-center rounded-md bg-[#18181b] px-2 py-1.5 text-[#52525b] transition-colors hover:bg-[#450a0a]/40 hover:text-[#ef4444] disabled:cursor-not-allowed disabled:opacity-40"
          aria-label={`Delete ${repo}@${branch}`}
        >
          <Trash2 className="size-3" />
        </button>
      </div>
    </div>
  );
}
