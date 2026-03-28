/**
 * ProjectsPanel Component
 * Lists all projects with project cards
 */

import React from 'react';
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
  isLoading: boolean;
}

export function ProjectsPanel({
  projects,
  selectedProjectId,
  onSelectProject,
  onRun,
  onStop,
  onGitSync,
  onDelete,
  isLoading,
}: ProjectsPanelProps) {
  if (projects.length === 0) {
    return (
      <div className="flex flex-1 items-center justify-center p-8">
        <p className="text-color-text-secondary">No projects configured</p>
      </div>
    );
  }

  return (
    <div className="flex-1 space-y-3 p-4">
      {projects.map((project) => (
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
        />
      ))}
    </div>
  );
}
