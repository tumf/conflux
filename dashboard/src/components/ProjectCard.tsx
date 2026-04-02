import React from 'react';
import { RefreshCw, Loader2, Trash2 } from 'lucide-react';
import { RemoteProject, ActiveCommand, RemoteSyncState } from '../api/types';

interface ProjectCardProps {
  project: RemoteProject;
  isSelected: boolean;
  onSelect: (projectId: string | null) => void;
  onGitSync: (projectId: string) => void;
  onDelete: (projectId: string) => void;
  isLoading: boolean;
  /** Whether git/sync is available (resolve_command configured on server) */
  syncAvailable: boolean;
  /** Active commands for this project */
  activeCommands?: ActiveCommand[];
}

const statusConfig: Record<string, { dot: string; text: string; bg: string }> = {
  idle: { dot: 'bg-[#52525b]', text: 'text-[#71717a]', bg: 'bg-[#18181b]' },
  running: { dot: 'bg-[#22c55e] animate-pulse', text: 'text-[#22c55e]', bg: 'bg-[#052e16]/40' },
  stopped: { dot: 'bg-[#ef4444]', text: 'text-[#ef4444]', bg: 'bg-[#450a0a]/40' },
};

const syncStateLabel: Record<RemoteSyncState, string> = {
  up_to_date: 'Up to date',
  ahead: 'Ahead',
  behind: 'Behind',
  diverged: 'Diverged',
  unknown: 'Unknown',
};

const syncStateClass: Record<RemoteSyncState, string> = {
  up_to_date: 'border-[#14532d] bg-[#052e16]/40 text-[#22c55e]',
  ahead: 'border-[#1d4ed8] bg-[#1e3a8a]/30 text-[#60a5fa]',
  behind: 'border-[#7c2d12] bg-[#431407]/40 text-[#fb923c]',
  diverged: 'border-[#7f1d1d] bg-[#450a0a]/40 text-[#f87171]',
  unknown: 'border-[#3f3f46] bg-[#18181b] text-[#a1a1aa]',
};

export function ProjectCard({
  project,
  isSelected,
  onSelect,
  onGitSync,
  onDelete,
  isLoading,
  syncAvailable,
  activeCommands = [],
}: ProjectCardProps) {
  const [repo, branch] = [project.repo, project.branch];
  const cfg = statusConfig[project.status] ?? statusConfig.idle;
  const syncState = project.sync_state ?? 'unknown';
  const syncCounts = `${project.ahead_count}↑ ${project.behind_count}↓`;

  // Check if base root is busy (sync operates on base root)
  const baseBusy = activeCommands.some(
    (cmd) => cmd.project_id === project.id && cmd.root === 'base'
  );
  const syncDisabled = isLoading || !syncAvailable || baseBusy;
  const nextSelection = isSelected ? null : project.id;

  return (
    <div
      onClick={() => onSelect(nextSelection)}
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
          onSelect(nextSelection);
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

      <div className="mb-2 flex items-center gap-2">
        <span
          className={`inline-flex items-center rounded border px-2 py-0.5 text-[11px] font-medium ${syncStateClass[syncState]}`}
          title={project.remote_check_error ?? undefined}
        >
          {syncStateLabel[syncState]}
        </span>
        <span className="text-[11px] text-[#a1a1aa]">{syncCounts}</span>
      </div>

      <div className="flex gap-1.5">
        <button
          onClick={(e) => { e.stopPropagation(); onGitSync(project.id); }}
          disabled={syncDisabled}
          className="flex flex-1 items-center justify-center gap-1 rounded-md bg-[#1e3a5f]/60 px-2 py-1.5 text-xs font-medium text-[#3b82f6] transition-colors hover:bg-[#1e3a5f]/80 disabled:cursor-not-allowed disabled:opacity-40"
          aria-label={`Sync ${repo}@${branch}`}
          title={
            !syncAvailable
              ? 'resolve_command is not configured'
              : baseBusy
                ? 'Sync is in progress'
                : undefined
          }
        >
          {baseBusy ? (
            <Loader2 className="size-3 animate-spin" />
          ) : (
            <RefreshCw className="size-3" />
          )}
          {baseBusy ? 'Syncing…' : 'Sync'}
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
