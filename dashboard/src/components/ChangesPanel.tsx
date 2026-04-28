import React, { useCallback, useMemo, useState } from 'react';
import { toast } from 'sonner';

import { APIError, toggleAllChangeSelection } from '../api/restClient';
import { RemoteChange, RemoteProject } from '../api/types';
import { ChangeRow } from './ChangeRow';

interface ChangesPanelProps {
  projects: RemoteProject[];
  selectedProjectId: string | null;
  onClickChange?: (changeId: string) => void;
  selectedChangeId?: string | null;
  onOptimisticSelectionChange?: (change: RemoteChange, selected: boolean) => void;
  onOptimisticSelectionRollback?: (change: RemoteChange) => void;
}

export function ChangesPanel({
  projects,
  selectedProjectId,
  onClickChange,
  selectedChangeId,
  onOptimisticSelectionChange,
  onOptimisticSelectionRollback,
}: ChangesPanelProps) {
  const [isBulkToggleLoading, setIsBulkToggleLoading] = useState(false);

  const project = useMemo(
    () => (selectedProjectId ? projects.find((p) => p.id === selectedProjectId) : undefined),
    [projects, selectedProjectId],
  );
  const projectChanges: RemoteChange[] = project?.changes ?? [];

  const selectableChanges = useMemo(
    () => projectChanges.filter((change) => change.status !== 'rejected'),
    [projectChanges],
  );
  const allSelectableSelected =
    selectableChanges.length > 0 && selectableChanges.every((change) => change.selected);

  const handleBulkToggle = useCallback(() => {
    if (!project || selectableChanges.length === 0 || isBulkToggleLoading) {
      return;
    }

    const nextSelected = !allSelectableSelected;
    selectableChanges.forEach((change) => {
      if (change.selected !== nextSelected) {
        onOptimisticSelectionChange?.(change, nextSelected);
      }
    });

    setIsBulkToggleLoading(true);
    toggleAllChangeSelection(project.id)
      .catch((error) => {
        selectableChanges.forEach((change) => {
          if (change.selected !== nextSelected) {
            onOptimisticSelectionRollback?.(change);
          }
        });

        const message =
          error instanceof APIError
            ? error.message
            : error instanceof Error
              ? error.message
              : String(error);
        toast.error(`Failed to update all selections: ${message}`);
      })
      .finally(() => {
        setIsBulkToggleLoading(false);
      });
  }, [
    allSelectableSelected,
    isBulkToggleLoading,
    onOptimisticSelectionChange,
    onOptimisticSelectionRollback,
    project,
    selectableChanges,
  ]);

  if (!selectedProjectId) {
    return (
      <div className="flex flex-1 items-center justify-center p-8">
        <p className="text-sm text-[#52525b]">Select a project to view changes</p>
      </div>
    );
  }

  if (projectChanges.length === 0) {
    return (
      <div className="flex flex-1 items-center justify-center p-8">
        <p className="text-sm text-[#52525b]">No changes</p>
      </div>
    );
  }

  return (
    <div className="space-y-2 p-3">
      <div className="flex items-center justify-end">
        <button
          type="button"
          onClick={handleBulkToggle}
          disabled={isBulkToggleLoading || selectableChanges.length === 0}
          className="rounded border border-[#3f3f46] px-2 py-1 text-xs text-[#a1a1aa] transition-colors hover:border-[#52525b] hover:text-[#fafafa] disabled:cursor-not-allowed disabled:opacity-50"
          aria-label={allSelectableSelected ? 'Deselect all changes' : 'Select all changes'}
        >
          {allSelectableSelected ? 'Deselect all' : 'Select all'}
        </button>
      </div>
      <div className="space-y-1.5">
        {projectChanges.map((change) => (
          <ChangeRow
            key={change.id}
            change={change}
            onClickChange={onClickChange}
            isSelected={selectedChangeId === change.id}
            onOptimisticSelectionChange={onOptimisticSelectionChange}
            onOptimisticSelectionRollback={onOptimisticSelectionRollback}
          />
        ))}
      </div>
    </div>
  );
}
