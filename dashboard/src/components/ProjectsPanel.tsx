import React from 'react';
import { RemoteProject } from '../api/types';
import { ProjectCard } from './ProjectCard';

interface ProjectsPanelProps {
  projects: RemoteProject[];
  selectedProjectId: string | null;
  onSelectProject: (projectId: string) => void;
  onGitSync: (projectId: string) => void;
  onDelete: (projectId: string) => void;
  isLoading: boolean;
  /** Whether git/sync is available (resolve_command configured on server) */
  syncAvailable: boolean;
}

export function ProjectsPanel({
  projects,
  selectedProjectId,
  onSelectProject,
  onGitSync,
  onDelete,
  isLoading,
  syncAvailable,
}: ProjectsPanelProps) {
  return (
    <div className="space-y-2 p-3">
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
