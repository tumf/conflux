import React, { useState, useCallback } from 'react';
import { Plus } from 'lucide-react';
import { Toaster, toast } from 'sonner';
import { Header } from './components/Header';
import { ProjectsPanel } from './components/ProjectsPanel';
import { ChangesPanel } from './components/ChangesPanel';
import { WorktreesPanel } from './components/WorktreesPanel';
import { LogsPanel } from './components/LogsPanel';
import { DeleteDialog } from './components/DeleteDialog';
import { DeleteWorktreeDialog } from './components/DeleteWorktreeDialog';
import { AddProjectDialog } from './components/AddProjectDialog';
import { CreateWorktreeDialog } from './components/CreateWorktreeDialog';
import { useAppStore } from './store/useAppStore';
import { useWebSocket } from './hooks/useWebSocket';
import {
  controlRun,
  controlStop,
  gitSync,
  deleteProject as deleteProjectAPI,
  addProject as addProjectAPI,
  createWorktree as createWorktreeAPI,
  deleteWorktree as deleteWorktreeAPI,
  mergeWorktree as mergeWorktreeAPI,
  refreshWorktrees as refreshWorktreesAPI,
  APIError,
} from './api/restClient';

type TabName = 'projects' | 'changes' | 'worktrees' | 'logs';
type DesktopCenterTab = 'changes' | 'worktrees';

function App() {
  const store = useAppStore();
  const [activeTab, setActiveTab] = useState<TabName>('projects');
  const [desktopCenterTab, setDesktopCenterTab] = useState<DesktopCenterTab>('changes');
  const [isLoading, setIsLoading] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<{ id: string; name: string } | null>(null);
  const [isAddProjectOpen, setIsAddProjectOpen] = useState(false);
  const [isCreateWorktreeOpen, setIsCreateWorktreeOpen] = useState(false);
  const [deleteWorktreeTarget, setDeleteWorktreeTarget] = useState<string | null>(null);

  useWebSocket({
    onStateUpdate: (state) => store.setFullState(state),
    onLogEntry: (entry) => store.appendLog(entry),
    onConnectionChange: (status) => store.setConnectionStatus(status),
    onLogEntry: (entry) => store.appendLog(entry),
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

  const handleDeleteClick = useCallback((projectId: string) => {
    const project = store.state.projects.find((p) => p.id === projectId);
    const name = project ? `${project.repo}/${project.branch}` : projectId;
    setDeleteTarget({ id: projectId, name });
  }, [store.state.projects]);

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

  const handleAddProject = useCallback(async (remoteUrl: string, branch: string) => {
    setIsLoading(true);
    try {
      await addProjectAPI(remoteUrl, branch);
      toast.success('Project added');
      setIsAddProjectOpen(false);
    } catch (err) {
      toast.error(`Failed to add project: ${err instanceof APIError ? err.message : String(err)}`);
    } finally {
      setIsLoading(false);
    }
  }, []);

  // Worktree handlers
  const handleCreateWorktree = useCallback(async (changeId: string) => {
    const projectId = store.state.selectedProjectId;
    if (!projectId) return;
    setIsLoading(true);
    try {
      await createWorktreeAPI(projectId, changeId);
      toast.success('Worktree created');
      setIsCreateWorktreeOpen(false);
      // Refresh worktree list
      const updated = await refreshWorktreesAPI(projectId);
      store.setWorktrees(projectId, updated);
    } catch (err) {
      toast.error(`Failed to create worktree: ${err instanceof APIError ? err.message : String(err)}`);
    } finally {
      setIsLoading(false);
    }
  }, [store]);

  const handleDeleteWorktreeClick = useCallback((branchName: string) => {
    setDeleteWorktreeTarget(branchName);
  }, []);

  const handleDeleteWorktreeConfirm = useCallback(async () => {
    const projectId = store.state.selectedProjectId;
    if (!projectId || !deleteWorktreeTarget) return;
    setIsLoading(true);
    try {
      await deleteWorktreeAPI(projectId, deleteWorktreeTarget);
      toast.success('Worktree deleted');
      setDeleteWorktreeTarget(null);
      // Refresh worktree list
      const updated = await refreshWorktreesAPI(projectId);
      store.setWorktrees(projectId, updated);
    } catch (err) {
      toast.error(`Failed to delete worktree: ${err instanceof APIError ? err.message : String(err)}`);
    } finally {
      setIsLoading(false);
    }
  }, [store, deleteWorktreeTarget]);

  const handleMergeWorktree = useCallback(async (branchName: string) => {
    const projectId = store.state.selectedProjectId;
    if (!projectId) return;
    setIsLoading(true);
    try {
      await mergeWorktreeAPI(projectId, branchName);
      toast.success('Branch merged successfully');
      // Refresh worktree list
      const updated = await refreshWorktreesAPI(projectId);
      store.setWorktrees(projectId, updated);
    } catch (err) {
      toast.error(`Failed to merge: ${err instanceof APIError ? err.message : String(err)}`);
    } finally {
      setIsLoading(false);
    }
  }, [store]);

  const handleRefreshWorktrees = useCallback(async () => {
    const projectId = store.state.selectedProjectId;
    if (!projectId) return;
    setIsLoading(true);
    try {
      const updated = await refreshWorktreesAPI(projectId);
      store.setWorktrees(projectId, updated);
    } catch (err) {
      toast.error(`Failed to refresh worktrees: ${err instanceof APIError ? err.message : String(err)}`);
    } finally {
      setIsLoading(false);
    }
  }, [store]);

  const selectedProject = store.state.projects.find(
    (p) => p.id === store.state.selectedProjectId,
  );
  const selectedProjectLogs = store.state.selectedProjectId
    ? store.state.logsByProjectId[store.state.selectedProjectId] || []
    : [];
  const selectedProjectWorktrees = store.state.selectedProjectId
    ? store.state.worktreesByProjectId[store.state.selectedProjectId] || []
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
    syncAvailable: store.state.syncAvailable,
  };

  return (
    <div className="flex h-screen flex-col bg-[#09090b] text-[#fafafa]">
      <Header connectionStatus={store.state.connectionStatus} />

      <div className="flex flex-1 overflow-hidden">
        {/* Desktop layout */}
        <aside className="hidden w-72 shrink-0 flex-col border-r border-[#27272a] md:flex">
          <div className="flex items-center justify-between border-b border-[#27272a] px-3 py-2">
            <span className="text-xs font-medium text-[#52525b] uppercase tracking-wider">Projects</span>
            <button
              onClick={() => setIsAddProjectOpen(true)}
              className="rounded p-0.5 text-[#52525b] transition-colors hover:text-[#6366f1]"
              aria-label="Add project"
            >
              <Plus className="size-4" />
            </button>
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
              {/* Tab switcher for Changes/Worktrees */}
              <div className="flex border-b border-[#27272a]">
                {(['changes', 'worktrees'] as DesktopCenterTab[]).map((tab) => (
                  <button
                    key={tab}
                    onClick={() => setDesktopCenterTab(tab)}
                    className={`flex-1 py-2 text-xs font-medium transition-colors ${
                      desktopCenterTab === tab
                        ? 'border-b-2 border-[#6366f1] text-[#fafafa]'
                        : 'text-[#52525b] hover:text-[#a1a1aa]'
                    }`}
                  >
                    {tab.charAt(0).toUpperCase() + tab.slice(1)}
                  </button>
                ))}
              </div>
              <div className="flex-1 overflow-y-auto">
                {desktopCenterTab === 'changes' ? (
                  <ChangesPanel
                    projects={store.state.projects}
                    selectedProjectId={store.state.selectedProjectId}
                  />
                ) : (
                  <WorktreesPanel
                    worktrees={selectedProjectWorktrees}
                    selectedProjectId={store.state.selectedProjectId}
                    onMerge={handleMergeWorktree}
                    onDelete={handleDeleteWorktreeClick}
                    onCreate={() => setIsCreateWorktreeOpen(true)}
                    onRefresh={handleRefreshWorktrees}
                    isLoading={isLoading}
                  />
                )}
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
            {(['projects', 'changes', 'worktrees', 'logs'] as TabName[]).map((tab) => (
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
            {activeTab === 'projects' && (
              <div className="flex h-full flex-col">
                <div className="flex items-center justify-between border-b border-[#27272a] px-3 py-2">
                  <span className="text-xs font-medium text-[#52525b] uppercase tracking-wider">Projects</span>
                  <button
                    onClick={() => setIsAddProjectOpen(true)}
                    className="rounded p-0.5 text-[#52525b] transition-colors hover:text-[#6366f1]"
                    aria-label="Add project"
                  >
                    <Plus className="size-4" />
                  </button>
                </div>
                <div className="flex-1 overflow-y-auto">
                  <ProjectsPanel {...panelProps} />
                </div>
              </div>
            )}
            {activeTab === 'changes' && (
              <ChangesPanel
                projects={store.state.projects}
                selectedProjectId={store.state.selectedProjectId}
              />
            )}
            {activeTab === 'worktrees' && (
              <WorktreesPanel
                worktrees={selectedProjectWorktrees}
                selectedProjectId={store.state.selectedProjectId}
                onMerge={handleMergeWorktree}
                onDelete={handleDeleteWorktreeClick}
                onCreate={() => setIsCreateWorktreeOpen(true)}
                onRefresh={handleRefreshWorktrees}
                isLoading={isLoading}
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

      <DeleteWorktreeDialog
        isOpen={deleteWorktreeTarget !== null}
        branchName={deleteWorktreeTarget || ''}
        onConfirm={handleDeleteWorktreeConfirm}
        onCancel={() => setDeleteWorktreeTarget(null)}
        isLoading={isLoading}
      />

      <AddProjectDialog
        isOpen={isAddProjectOpen}
        onSubmit={handleAddProject}
        onCancel={() => setIsAddProjectOpen(false)}
        isLoading={isLoading}
      />

      <CreateWorktreeDialog
        isOpen={isCreateWorktreeOpen}
        onSubmit={handleCreateWorktree}
        onCancel={() => setIsCreateWorktreeOpen(false)}
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
