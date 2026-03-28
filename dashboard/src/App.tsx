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
  const [deleteTarget, setDeleteTarget] = useState<{ id: string; name: string } | null>(null);

  useWebSocket({
    onStateUpdate: (state) => store.setFullState(state),
    onConnectionChange: (status) => store.setConnectionStatus(status),
    onError: (error) => {
      console.error('WebSocket error:', error);
      toast.error(`Connection error: ${error.message}`);
    },
  });

  const handleRun = useCallback(async (projectId: string) => {
    setIsLoading(true);
    try {
      await controlRun(projectId);
      toast.success('Project started');
    } catch (err) {
      toast.error(`Failed to start: ${err instanceof APIError ? err.message : String(err)}`);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const handleStop = useCallback(async (projectId: string) => {
    setIsLoading(true);
    try {
      await controlStop(projectId);
      toast.success('Project stopped');
    } catch (err) {
      toast.error(`Failed to stop: ${err instanceof APIError ? err.message : String(err)}`);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const handleGitSync = useCallback(async (projectId: string) => {
    setIsLoading(true);
    try {
      await gitSync(projectId);
      toast.success('Git sync completed');
    } catch (err) {
      toast.error(`Failed to sync: ${err instanceof APIError ? err.message : String(err)}`);
    } finally {
      setIsLoading(false);
    }
  }, []);

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
      toast.error(`Failed to delete: ${err instanceof APIError ? err.message : String(err)}`);
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

  const panelProps = {
    projects: store.state.projects,
    selectedProjectId: store.state.selectedProjectId,
    onSelectProject: store.selectProject,
    onRun: handleRun,
    onStop: handleStop,
    onGitSync: handleGitSync,
    onDelete: handleDeleteClick,
    isLoading,
  };

  return (
    <div className="flex h-screen flex-col bg-[#09090b] text-[#fafafa]">
      <Header connectionStatus={store.state.connectionStatus} />

      <div className="flex flex-1 overflow-hidden">
        {/* Desktop layout */}
        <aside className="hidden w-72 shrink-0 flex-col border-r border-[#27272a] md:flex">
          <div className="border-b border-[#27272a] px-3 py-2">
            <span className="text-xs font-medium text-[#52525b] uppercase tracking-wider">Projects</span>
          </div>
          <div className="flex-1 overflow-y-auto">
            <ProjectsPanel {...panelProps} />
          </div>
        </aside>

        <main className="hidden flex-col md:flex md:flex-1 overflow-hidden">
          <div className="border-b border-[#27272a] px-4 py-2.5">
            {selectedProject ? (
              <div className="flex items-center gap-1.5">
                <span className="text-sm font-medium text-[#fafafa]">{selectedProject.repo}</span>
                <span className="text-[#3f3f46]">/</span>
                <span className="text-sm text-[#71717a]">{selectedProject.branch}</span>
              </div>
            ) : (
              <span className="text-sm text-[#52525b]">Select a project</span>
            )}
          </div>

          <div className="flex flex-1 overflow-hidden">
            <div className="flex w-72 shrink-0 flex-col border-r border-[#27272a]">
              <div className="border-b border-[#27272a] px-3 py-2">
                <span className="text-xs font-medium text-[#52525b] uppercase tracking-wider">Changes</span>
              </div>
              <div className="flex-1 overflow-y-auto">
                <ChangesPanel
                  changes={store.state.changes}
                  selectedProjectId={store.state.selectedProjectId}
                />
              </div>
            </div>

            <div className="flex flex-1 flex-col overflow-hidden">
              <div className="border-b border-[#27272a] px-3 py-2">
                <span className="text-xs font-medium text-[#52525b] uppercase tracking-wider">Logs</span>
              </div>
              <div className="flex-1 overflow-y-auto">
                <LogsPanel
                  logs={selectedProjectLogs}
                  selectedProjectId={store.state.selectedProjectId}
                />
              </div>
            </div>
          </div>
        </main>

        {/* Mobile layout */}
        <div className="flex flex-1 flex-col md:hidden">
          <div className="flex border-b border-[#27272a]">
            {(['projects', 'changes', 'logs'] as TabName[]).map((tab) => (
              <button
                key={tab}
                onClick={() => setActiveTab(tab)}
                className={`flex-1 py-2.5 text-xs font-medium transition-colors ${
                  activeTab === tab
                    ? 'border-b-2 border-[#6366f1] text-[#fafafa]'
                    : 'text-[#52525b] hover:text-[#a1a1aa]'
                }`}
              >
                {tab.charAt(0).toUpperCase() + tab.slice(1)}
              </button>
            ))}
          </div>

          <div className="flex-1 overflow-hidden">
            {activeTab === 'projects' && <ProjectsPanel {...panelProps} />}
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

      <Toaster
        position="bottom-right"
        theme="dark"
        toastOptions={{
          style: {
            background: '#18181b',
            border: '1px solid #27272a',
            color: '#fafafa',
          },
        }}
      />
    </div>
  );
}

export default App;
