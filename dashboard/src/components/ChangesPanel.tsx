import React from 'react';
import { RemoteChange, RemoteProject } from '../api/types';
import { ChangeRow } from './ChangeRow';

interface ChangesPanelProps {
  projects: RemoteProject[];
  selectedProjectId: string | null;
  onClickChange?: (changeId: string) => void;
  selectedChangeId?: string | null;
}

export function ChangesPanel({ projects, selectedProjectId, onClickChange, selectedChangeId }: ChangesPanelProps) {
  if (!selectedProjectId) {
    return (
      <div className="flex flex-1 items-center justify-center p-8">
        <p className="text-sm text-[#52525b]">Select a project to view changes</p>
      </div>
    );
  }

  const project = projects.find((p) => p.id === selectedProjectId);
  const projectChanges: RemoteChange[] = project?.changes ?? [];

  if (projectChanges.length === 0) {
    return (
      <div className="flex flex-1 items-center justify-center p-8">
        <p className="text-sm text-[#52525b]">No changes</p>
      </div>
    );
  }

  return (
    <div className="space-y-1.5 p-3">
      {projectChanges.map((change) => (
        <ChangeRow
          key={change.id}
          change={change}
          onClickChange={onClickChange}
          isSelected={selectedChangeId === change.id}
        />
      ))}
    </div>
  );
}
