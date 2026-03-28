import React from 'react';
import { RemoteChange } from '../api/types';
import { ChangeRow } from './ChangeRow';

interface ChangesPanelProps {
  changes: RemoteChange[];
  selectedProjectId: string | null;
}

export function ChangesPanel({ changes, selectedProjectId }: ChangesPanelProps) {
  if (!selectedProjectId) {
    return (
      <div className="flex flex-1 items-center justify-center p-8">
        <p className="text-sm text-[#52525b]">Select a project to view changes</p>
      </div>
    );
  }

  const projectChanges = changes.filter(
    (change) => change.project === selectedProjectId,
  );

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
        <ChangeRow key={change.id} change={change} />
      ))}
    </div>
  );
}
