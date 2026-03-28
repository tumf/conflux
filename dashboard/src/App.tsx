/**
 * Main App Component
 * Orchestrates dashboard with WebSocket connection and state management
 */

import React, { useState, useCallback } from 'react';
import { Toaster, toast } from 'sonner';
import { Header } from './components/Header';
import { ProjectsPanel } from './components/ProjectsPanel';
import { ChangesPanel } from './components/ChangesPanel';
import { LogsPanel } from './components/LogsPanel';
import { DeleteDialog } from './components/DeleteDialog';
import { useAppStore } from './store/useAppStore';
import { useWebSocket } from './hooks/useWebSocket';
import {
  controlRun,
  controlStop,
  gitSync,
  deleteProject as deleteProjectAPI,
  APIError,
} from './api/restClient';

type TabName = 'projects' | 'changes' | 'logs';

function App() {
  const store = useAppStore();
  const [activeTab, setActiveTab] = useState<TabName>('projects');
  const [isLoading, setIsLoading] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<{
    id: string;
    name: string;
  } | null>(null);

  // Setup WebSocket connection
  useWebSocket({
    onStateUpdate: (state) => {
      store.setFullState(state);
    },
    onConnectionChange: (status) => {
      store.setConnectionStatus(status);
    },
    onError: (error) => {
      console.error('WebSocket error:', error);
      toast.error(`Connection error: ${error.message}`);
    },
  });

  const handleRun = useCallback(
    async (projectId: string) => {
      setIsLoading(true);
      try {
        await controlRun(projectId);
        toast.success('Project started');
      } catch (err) {
        const message = err instanceof APIError ? err.message : String(err);
        toast.error(`Failed to start project: ${message}`);
      } finally {
        setIsLoading(false);
      }
    },
    [],
  );

  const handleStop = useCallback(
    async (projectId: string) => {
      setIsLoading(true);
      try {
        await controlStop(projectId);
        toast.success('Project stopped');
      } catch (err) {
        const message = err instanceof APIError ? err.message : String(err);
        toast.error(`Failed to stop project: ${message}`);
      } finally {
        setIsLoading(false);
      }
    },
    [],
  );

  const handleGitSync = useCallback(
    async (projectId: string) => {
      setIsLoading(true);
      try {
        await gitSync(projectId);
        toast.success('Git sync completed');
      } catch (err) {
        const message = err instanceof APIError ? err.message : String(err);
        toast.error(`Failed to sync: ${message}`);
      } finally {
        setIsLoading(false);
      }
    },
    [],
  );

  const handleDeleteClick = useCallback((projectId: string, projectName: string) => {
    setDeleteTarget({ id: projectId, name: projectName });
  }, []);

  const handleDeleteConfirm = useCallback(async () => {
    if (!deleteTarget) return;

    setIsLoading(true);
    try {
      await deleteProjectAPI(deleteTarget.id);
      toast.success('Project deleted');
      setDeleteTarget(null);
    } catch (err) {
      const message = err instanceof APIError ? err.message : String(err);
      toast.error(`Failed to delete project: ${message}`);
    } finally {
      setIsLoading(false);
    }
  }, [deleteTarget]);

  const selectedProject = store.state.projects.find(
    (p) => p.id === store.state.selectedProjectId,
  );
  const selectedProjectLogs = store.state.selectedProjectId
    ? store.state.logsByProjectId[store.state.selectedProjectId] || []
    : [];

  return (
    <div className="flex h-screen flex-col bg-color-background text-color-text">
      <Header connectionStatus={store.state.connectionStatus} />

      <div className="flex flex-1 overflow-hidden">
        {/* Desktop: Side-by-side layout */}
        <div className="hidden flex-col border-r border-color-border md:flex md:w-1/3">
          <div className="flex-1 overflow-y-auto">
            <ProjectsPanel
              projects={store.state.projects}
              selectedProjectId={store.state.selectedProjectId}
              onSelectProject={store.selectProject}
              onRun={handleRun}
              onStop={handleStop}
              onGitSync={handleGitSync}
              onDelete={handleDeleteClick}
              isLoading={isLoading}
            />
          </div>
        </div>

        <div className="hidden flex-col md:flex md:flex-1">
          <div className="border-b border-color-border px-4 py-2">
            <h2 className="text-lg font-semibold text-color-text">
              {selectedProject
                ? `${selectedProject.repo}@${selectedProject.branch}`
                : 'Select a project'}
            </h2>
          </div>
          <div className="flex flex-1 gap-4 p-4">
            <div className="flex-1">
              <h3 className="mb-2 text-sm font-semibold text-color-text-secondary">Changes</h3>
              <div className="h-full overflow-y-auto rounded border border-color-border">
                <ChangesPanel
                  changes={store.state.changes}
                  selectedProjectId={store.state.selectedProjectId}
                />
              </div>
            </div>
            <div className="flex-1">
              <h3 className="mb-2 text-sm font-semibold text-color-text-secondary">Logs</h3>
              <div className="h-full overflow-y-auto rounded border border-color-border">
                <LogsPanel
                  logs={selectedProjectLogs}
                  selectedProjectId={store.state.selectedProjectId}
                />
              </div>
            </div>
          </div>
        </div>

        {/* Mobile: Tab layout */}
        <div className="flex flex-1 flex-col md:hidden">
          <div className="flex border-b border-color-border">
            {(['projects', 'changes', 'logs'] as TabName[]).map((tab) => (
              <button
                key={tab}
                onClick={() => setActiveTab(tab)}
                className={`flex-1 border-b-2 px-4 py-2 text-sm font-semibold ${
                  activeTab === tab
                    ? 'border-color-accent text-color-accent'
                    : 'border-transparent text-color-text-secondary hover:text-color-text'
                }`}
              >
                {tab.charAt(0).toUpperCase() + tab.slice(1)}
              </button>
            ))}
          </div>

          <div className="flex-1 overflow-hidden">
            {activeTab === 'projects' && (
              <ProjectsPanel
                projects={store.state.projects}
                selectedProjectId={store.state.selectedProjectId}
                onSelectProject={store.selectProject}
                onRun={handleRun}
                onStop={handleStop}
                onGitSync={handleGitSync}
                onDelete={handleDeleteClick}
                isLoading={isLoading}
              />
            )}
            {activeTab === 'changes' && (
              <ChangesPanel
                changes={store.state.changes}
                selectedProjectId={store.state.selectedProjectId}
              />
            )}
            {activeTab === 'logs' && (
              <LogsPanel
                logs={selectedProjectLogs}
                selectedProjectId={store.state.selectedProjectId}
              />
            )}
          </div>
        </div>
      </div>

      <DeleteDialog
        isOpen={deleteTarget !== null}
        projectName={deleteTarget?.name || ''}
        onConfirm={handleDeleteConfirm}
        onCancel={() => setDeleteTarget(null)}
        isLoading={isLoading}
      />

      <Toaster position="bottom-right" />
    </div>
  );
}

export default App;
