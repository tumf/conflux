import React from 'react';
import { Plus } from 'lucide-react';
import { RemoteProject } from '../api/types';
import { ProjectCard } from './ProjectCard';

interface ProjectsPanelProps {
  projects: RemoteProject[];
  selectedProjectId: string | null;
  onSelectProject: (projectId: string) => void;
  onRun: (projectId: string) => void;
  onStop: (projectId: string) => void;
  onGitSync: (projectId: string) => void;
  onDelete: (projectId: string) => void;
  onAddProject: () => void;
  isLoading: boolean;
  /** Whether git/sync is available (resolve_command configured on server) */
  syncAvailable: boolean;
}

export function ProjectsPanel({
  projects,
  selectedProjectId,
  onSelectProject,
  onRun,
  onStop,
  onGitSync,
  onDelete,
  onAddProject,
  isLoading,
  syncAvailable,
}: ProjectsPanelProps) {
  return (
    <div className="space-y-2 p-3">
      <button
        onClick={onAddProject}
        className="flex w-full items-center justify-center gap-1.5 rounded-lg border border-dashed border-[#27272a] px-3 py-2 text-sm text-[#71717a] transition-colors hover:border-[#6366f1] hover:text-[#6366f1]"
      >
        <Plus className="size-4" />
        Add Project
      </button>

      {projects.length === 0 ? (
        <div className="flex items-center justify-center p-6">
          <p className="text-sm text-[#52525b]">No projects configured</p>
        </div>
      ) : (
        projects.map((project) => (
          <ProjectCard
            key={project.id}
            project={project}
            isSelected={selectedProjectId === project.id}
            onSelect={onSelectProject}
            onRun={onRun}
            onStop={onStop}
            onGitSync={onGitSync}
            onDelete={onDelete}
            isLoading={isLoading}
            syncAvailable={syncAvailable}
          />
        ))
      )}
    </div>
  );
}
