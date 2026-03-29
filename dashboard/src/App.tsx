import React, { useState, useCallback } from 'react';
import { Plus } from 'lucide-react';
import { Toaster, toast } from 'sonner';
import { Header } from './components/Header';
import { ProjectsPanel } from './components/ProjectsPanel';
import { ChangesPanel } from './components/ChangesPanel';
import { WorktreesPanel } from './components/WorktreesPanel';
import { LogsPanel } from './components/LogsPanel';
import { FileViewPanel } from './components/FileViewPanel';
import { DeleteDialog } from './components/DeleteDialog';
import { DeleteWorktreeDialog } from './components/DeleteWorktreeDialog';
import { AddProjectDialog } from './components/AddProjectDialog';
import { CreateWorktreeDialog } from './components/CreateWorktreeDialog';
import { ProposalChat } from './components/ProposalChat';
import { ProposalSessionTabs } from './components/ProposalSessionTabs';
import { CloseSessionDialog } from './components/CloseSessionDialog';
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
  createProposalSession as createProposalSessionAPI,
  listProposalSessions as listProposalSessionsAPI,
  deleteProposalSession as deleteProposalSessionAPI,
  mergeProposalSession as mergeProposalSessionAPI,
  APIError,
} from './api/restClient';

type TabName = 'projects' | 'changes' | 'worktrees' | 'logs' | 'files';
type DesktopCenterTab = 'changes' | 'worktrees';
type DesktopRightTab = 'logs' | 'files';

function App() {
  const store = useAppStore();
  const [activeTab, setActiveTab] = useState<TabName>('projects');
  const [desktopCenterTab, setDesktopCenterTab] = useState<DesktopCenterTab>('changes');
  const [desktopRightTab, setDesktopRightTab] = useState<DesktopRightTab>('logs');
  const [isLoading, setIsLoading] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<{ id: string; name: string } | null>(null);
  const [isAddProjectOpen, setIsAddProjectOpen] = useState(false);
  const [isCreateWorktreeOpen, setIsCreateWorktreeOpen] = useState(false);
  const [deleteWorktreeTarget, setDeleteWorktreeTarget] = useState<string | null>(null);
  const [closeSessionTarget, setCloseSessionTarget] = useState<string | null>(null);

  useWebSocket({
    onStateUpdate: (state) => store.setFullState(state),
    onLogEntry: (entry) => store.appendLog(entry),
    onConnectionChange: (status) => store.setConnectionStatus(status),
    onError: (error) => {
      console.error('WebSocket error:', error);
      toast.error(`Connection error: ${error.message}`);
    },
  });

  // Global Run handler (starts orchestration across all projects)
  const handleRun = useCallback(async () => {
    setIsLoading(true);
    try {
      await controlRun();
      toast.success('Orchestration started');
    } catch (err) {
      toast.error(`Failed to start: ${err instanceof APIError ? err.message : String(err)}`);
    } finally {
      setIsLoading(false);
    }
  }, []);

  // Global Stop handler (stops orchestration across all projects)
  const handleStop = useCallback(async () => {
    setIsLoading(true);
    try {
      await controlStop();
      toast.success('Orchestration stopped');
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
      if (store.state.fileBrowseContext?.type === 'worktree' && store.state.fileBrowseContext.worktreeBranch === deleteWorktreeTarget) {
        store.setFileBrowseContext(null);
      }
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

  const handleClickChange = useCallback((changeId: string) => {
    store.setFileBrowseContext({ type: 'change', changeId });
    setDesktopRightTab('files');
    setActiveTab('files');
  }, [store]);

  const handleClickWorktree = useCallback((branch: string) => {
    store.setFileBrowseContext({ type: 'worktree', worktreeBranch: branch });
    setDesktopRightTab('files');
    setActiveTab('files');
    setDesktopCenterTab('worktrees');
  }, [store]);

  // ─── Proposal Session Handlers ────────────────────────────────────────────

  const handleCreateProposalSession = useCallback(async () => {
    const projectId = store.state.selectedProjectId;
    if (!projectId) return;
    setIsLoading(true);
    try {
      const session = await createProposalSessionAPI(projectId);
      store.addProposalSession(projectId, session);
      store.setActiveProposalSession(session.id);
      toast.success('Proposal session created');
    } catch (err) {
      toast.error(`Failed to create session: ${err instanceof APIError ? err.message : String(err)}`);
    } finally {
      setIsLoading(false);
    }
  }, [store]);

  const handleMergeProposalSession = useCallback(async () => {
    const projectId = store.state.selectedProjectId;
    const sessionId = store.state.activeProposalSessionId;
    if (!projectId || !sessionId) return;
    setIsLoading(true);
    try {
      await mergeProposalSessionAPI(projectId, sessionId);
      store.removeProposalSession(projectId, sessionId);
      toast.success('Session merged successfully');
    } catch (err) {
      toast.error(`Failed to merge: ${err instanceof APIError ? err.message : String(err)}`);
    } finally {
      setIsLoading(false);
    }
  }, [store]);

  const handleCloseProposalSession = useCallback(() => {
    const sessionId = store.state.activeProposalSessionId;
    if (!sessionId) return;

    // Find the session to check if dirty
    const projectId = store.state.selectedProjectId;
    if (!projectId) return;
    const sessions = store.state.proposalSessionsByProjectId[projectId] || [];
    const session = sessions.find((s) => s.id === sessionId);

    if (session?.is_dirty) {
      setCloseSessionTarget(sessionId);
    } else {
      handleForceCloseSession(sessionId);
    }
  }, [store]);

  const handleForceCloseSession = useCallback(async (sessionId?: string) => {
    const projectId = store.state.selectedProjectId;
    const targetId = sessionId || closeSessionTarget;
    if (!projectId || !targetId) return;
    setIsLoading(true);
    try {
      await deleteProposalSessionAPI(projectId, targetId, true);
      store.removeProposalSession(projectId, targetId);
      setCloseSessionTarget(null);
      toast.success('Session closed');
    } catch (err) {
      toast.error(`Failed to close session: ${err instanceof APIError ? err.message : String(err)}`);
    } finally {
      setIsLoading(false);
    }
  }, [store, closeSessionTarget]);

  const handleBackFromProposal = useCallback(() => {
    store.setActiveProposalSession(null);
  }, [store]);

  // Load proposal sessions when project is selected
  const handleSelectProjectWithSessions = useCallback((projectId: string | null) => {
    store.selectProject(projectId);
    store.setActiveProposalSession(null);
    if (projectId) {
      listProposalSessionsAPI(projectId)
        .then((sessions) => store.setProposalSessions(projectId, sessions))
        .catch((err) => console.error('Failed to load proposal sessions:', err));
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
  const allLogs = Object.values(store.state.logsByProjectId)
    .flat()
    .sort((a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime());
  const selectedProjectLogs = store.state.selectedProjectId
    ? store.state.logsByProjectId[store.state.selectedProjectId] || []
    : allLogs;
  const selectedProjectWorktrees = store.state.selectedProjectId
    ? store.state.worktreesByProjectId[store.state.selectedProjectId] || []
    : [];

  // Derived state for proposal sessions
  const currentProjectSessions = store.state.selectedProjectId
    ? store.state.proposalSessionsByProjectId[store.state.selectedProjectId] || []
    : [];
  const activeProposalSession = currentProjectSessions.find(
    (s) => s.id === store.state.activeProposalSessionId,
  );
  const activeSessionMessages = store.state.activeProposalSessionId
    ? store.state.chatMessagesBySessionId[store.state.activeProposalSessionId] || []
    : [];

  // Get close target session for dialog
  const closeTargetSession = closeSessionTarget
    ? currentProjectSessions.find((s) => s.id === closeSessionTarget)
    : null;

  const panelProps = {
    projects: store.state.projects,
    selectedProjectId: store.state.selectedProjectId,
    onSelectProject: handleSelectProjectWithSessions,
    onGitSync: handleGitSync,
    onDelete: handleDeleteClick,
    isLoading,
    syncAvailable: store.state.syncAvailable,
    activeCommands: store.state.activeCommands,
  };

  // Active commands for the currently selected project
  const selectedProjectActiveCommands = store.state.activeCommands.filter(
    (cmd) => cmd.project_id === store.state.selectedProjectId
  );

  return (
    <div className="flex h-screen flex-col bg-[#09090b] text-[#fafafa]">
      <Header
        connectionStatus={store.state.connectionStatus}
        orchestrationStatus={store.state.orchestrationStatus}
        onRun={handleRun}
        onStop={handleStop}
        isLoading={isLoading}
      />

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
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-1.5">
                  <span className="text-sm font-medium text-[#fafafa]">{selectedProject.repo}</span>
                  <span className="text-[#3f3f46]">/</span>
                  <span className="text-sm text-[#71717a]">{selectedProject.branch}</span>
                </div>
                {!activeProposalSession && (
                  <button
                    onClick={handleCreateProposalSession}
                    disabled={isLoading}
                    className="flex items-center gap-1.5 rounded-md bg-[#6366f1] px-2.5 py-1 text-xs font-medium text-white transition-colors hover:bg-[#4f46e5] disabled:opacity-50"
                  >
                    <Plus className="size-3" />
                    Add Proposal
                  </button>
                )}
              </div>
            ) : (
              <span className="text-sm text-[#52525b]">Select a project</span>
            )}
          </div>

          {/* Proposal session tabs */}
          {currentProjectSessions.length > 0 && (
            <ProposalSessionTabs
              sessions={currentProjectSessions}
              activeSessionId={store.state.activeProposalSessionId}
              onSelectSession={store.setActiveProposalSession}
              onCreateSession={handleCreateProposalSession}
              onCloseSession={(sid) => {
                const s = currentProjectSessions.find((x) => x.id === sid);
                if (s?.is_dirty) {
                  setCloseSessionTarget(sid);
                } else {
                  handleForceCloseSession(sid);
                }
              }}
            />
          )}

          {/* Show ProposalChat when a session is active, otherwise show normal panels */}
          {activeProposalSession && store.state.selectedProjectId ? (
            <ProposalChat
              projectId={store.state.selectedProjectId}
              session={activeProposalSession}
              messages={activeSessionMessages}
              streamingContent={store.state.streamingContent}
              activeElicitation={store.state.activeElicitation}
              isAgentResponding={store.state.isAgentResponding}
              onBack={handleBackFromProposal}
              onMerge={handleMergeProposalSession}
              onClose={handleCloseProposalSession}
              onAppendMessage={store.appendChatMessage}
              onStreamingChunk={store.appendStreamingChunk}
              onToolCallStart={store.updateToolCall}
              onToolCallUpdate={store.updateToolCallStatus}
                onElicitation={store.setElicitation}
                onClickChange={handleClickChange}
                isLoading={isLoading}
              />
          ) : (
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
                      onClickChange={handleClickChange}
                      selectedChangeId={store.state.fileBrowseContext?.type === 'change' ? store.state.fileBrowseContext.changeId : null}
                    />
                  ) : (
                    <WorktreesPanel
                      worktrees={selectedProjectWorktrees}
                      selectedProjectId={store.state.selectedProjectId}
                      onMerge={handleMergeWorktree}
                      onDelete={handleDeleteWorktreeClick}
                      onCreate={() => setIsCreateWorktreeOpen(true)}
                      onRefresh={handleRefreshWorktrees}
                      onClickWorktree={handleClickWorktree}
                      selectedWorktreeBranch={store.state.fileBrowseContext?.type === 'worktree' ? store.state.fileBrowseContext.worktreeBranch : null}
                      isLoading={isLoading}
                      activeCommands={selectedProjectActiveCommands}
                    />
                  )}
                </div>
              </div>

              <div className="flex flex-1 flex-col overflow-hidden">
                {/* Right pane tab switcher: Logs / Files (Logs hidden when worktree selected) */}
                {store.state.fileBrowseContext?.type === 'worktree' ? (
                  <>
                    <div className="flex border-b border-[#27272a]">
                      <div className="flex-1 py-2 text-xs font-medium border-b-2 border-[#6366f1] text-[#fafafa] text-center">
                        Files
                      </div>
                    </div>
                    <div className="flex flex-1 overflow-hidden">
                      <FileViewPanel
                        projectId={store.state.selectedProjectId}
                        context={store.state.fileBrowseContext}
                      />
                    </div>
                  </>
                ) : (
                  <>
                    <div className="flex border-b border-[#27272a]">
                      {(['logs', 'files'] as DesktopRightTab[]).map((tab) => (
                        <button
                          key={tab}
                          onClick={() => setDesktopRightTab(tab)}
                          className={`flex-1 py-2 text-xs font-medium transition-colors ${
                            desktopRightTab === tab
                              ? 'border-b-2 border-[#6366f1] text-[#fafafa]'
                              : 'text-[#52525b] hover:text-[#a1a1aa]'
                          }`}
                        >
                          {tab.charAt(0).toUpperCase() + tab.slice(1)}
                        </button>
                      ))}
                    </div>
                    <div className="flex flex-1 overflow-hidden">
                      {desktopRightTab === 'logs' ? (
                        <LogsPanel
                          logs={selectedProjectLogs}
                          selectedProjectId={store.state.selectedProjectId}
                        />
                      ) : (
                        <FileViewPanel
                          projectId={store.state.selectedProjectId}
                          context={store.state.fileBrowseContext}
                        />
                      )}
                    </div>
                  </>
                )}
              </div>
            </div>
          )}
        </main>

        {/* Mobile layout */}
        <div className="flex flex-1 flex-col md:hidden">
          <div className="flex border-b border-[#27272a]">
            {((['projects', 'changes', 'worktrees', 'logs', 'files'] as TabName[])
              .filter((tab) => !(tab === 'logs' && store.state.fileBrowseContext?.type === 'worktree'))
            ).map((tab) => (
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
                onClickChange={handleClickChange}
                selectedChangeId={store.state.fileBrowseContext?.type === 'change' ? store.state.fileBrowseContext.changeId : null}
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
                onClickWorktree={handleClickWorktree}
                selectedWorktreeBranch={store.state.fileBrowseContext?.type === 'worktree' ? store.state.fileBrowseContext.worktreeBranch : null}
                isLoading={isLoading}
                activeCommands={selectedProjectActiveCommands}
              />
            )}
            {activeTab === 'logs' && (
              <LogsPanel
                logs={selectedProjectLogs}
                selectedProjectId={store.state.selectedProjectId}
              />
            )}
            {activeTab === 'files' && (
              <FileViewPanel
                projectId={store.state.selectedProjectId}
                context={store.state.fileBrowseContext}
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

      <CloseSessionDialog
        isOpen={closeSessionTarget !== null}
        uncommittedFiles={closeTargetSession?.uncommitted_files || []}
        onForceClose={() => handleForceCloseSession()}
        onCancel={() => setCloseSessionTarget(null)}
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
