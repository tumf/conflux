/**
 * ChangesPanel Component
 * Displays changes for the selected project
 */

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
        <p className="text-color-text-secondary">Select a project to view changes</p>
      </div>
    );
  }

  const projectChanges = changes.filter(
    (change) => change.project_id === selectedProjectId,
  );

  if (projectChanges.length === 0) {
    return (
      <div className="flex flex-1 items-center justify-center p-8">
        <p className="text-color-text-secondary">No changes for this project</p>
      </div>
    );
  }

  return (
    <div className="flex-1 space-y-2 p-4">
      {projectChanges.map((change) => (
        <ChangeRow key={change.id} change={change} />
      ))}
    </div>
  );
}
