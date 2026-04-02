import { FileBrowseContext, RemoteProject, WorktreeInfo } from '../api/types';

export const UI_STATE_KEYS = {
  selectedProjectId: 'selected_project_id',
  activeProposalSessionId: 'active_proposal_session_id',
  fileBrowseContext: 'file_browse_context',
  desktopCenterTab: 'desktop_center_tab',
  desktopRightTab: 'desktop_right_tab',
  mobileActiveTab: 'mobile_active_tab',
} as const;

export type TabName = 'projects' | 'changes' | 'worktrees' | 'logs' | 'files';
export type DesktopCenterTab = 'changes' | 'worktrees';
export type DesktopRightTab = 'logs' | 'files';

interface TabState {
  desktopCenterTab?: DesktopCenterTab;
  desktopRightTab?: DesktopRightTab;
  mobileActiveTab?: TabName;
}

export function serializeFileBrowseContext(context: FileBrowseContext): string {
  return JSON.stringify(context);
}

export function parseFileBrowseContext(raw: string): FileBrowseContext | null {
  try {
    const parsed = JSON.parse(raw) as FileBrowseContext;
    if (parsed.type === 'change' && typeof parsed.changeId === 'string' && parsed.changeId.length > 0) {
      return { type: 'change', changeId: parsed.changeId };
    }
    if (parsed.type === 'worktree' && typeof parsed.worktreeBranch === 'string' && parsed.worktreeBranch.length > 0) {
      return { type: 'worktree', worktreeBranch: parsed.worktreeBranch };
    }
    return null;
  } catch {
    return null;
  }
}

export function parsePersistedTabState(uiState: Record<string, string> | undefined): TabState {
  const parsed: TabState = {};

  const desktopCenterTab = uiState?.[UI_STATE_KEYS.desktopCenterTab];
  if (desktopCenterTab === 'changes' || desktopCenterTab === 'worktrees') {
    parsed.desktopCenterTab = desktopCenterTab;
  }

  const desktopRightTab = uiState?.[UI_STATE_KEYS.desktopRightTab];
  if (desktopRightTab === 'logs' || desktopRightTab === 'files') {
    parsed.desktopRightTab = desktopRightTab;
  }

  const mobileActiveTab = uiState?.[UI_STATE_KEYS.mobileActiveTab];
  if (mobileActiveTab === 'projects' || mobileActiveTab === 'changes' || mobileActiveTab === 'worktrees' || mobileActiveTab === 'logs' || mobileActiveTab === 'files') {
    parsed.mobileActiveTab = mobileActiveTab;
  }

  return parsed;
}

interface ResolvePersistedBrowseSelectionParams {
  uiState: Record<string, string> | undefined;
  selectedProjectId: string | null;
  projects: RemoteProject[];
  worktreesByProjectId: Record<string, WorktreeInfo[]> | undefined;
}

export type ResolvePersistedBrowseSelectionResult =
  | { status: 'none' }
  | { status: 'defer' }
  | { status: 'stale'; keysToClear: string[] }
  | {
      status: 'restored';
      context: FileBrowseContext;
      tabs: {
        desktopCenterTab: DesktopCenterTab;
        desktopRightTab: DesktopRightTab;
        mobileActiveTab: TabName;
      };
    };

const STALE_BROWSE_KEYS = [
  UI_STATE_KEYS.fileBrowseContext,
  UI_STATE_KEYS.desktopCenterTab,
  UI_STATE_KEYS.desktopRightTab,
  UI_STATE_KEYS.mobileActiveTab,
];

export function resolvePersistedBrowseSelection({
  uiState,
  selectedProjectId,
  projects,
  worktreesByProjectId,
}: ResolvePersistedBrowseSelectionParams): ResolvePersistedBrowseSelectionResult {
  const rawContext = uiState?.[UI_STATE_KEYS.fileBrowseContext];
  if (!rawContext) {
    return { status: 'none' };
  }

  if (!selectedProjectId) {
    return { status: 'stale', keysToClear: STALE_BROWSE_KEYS };
  }

  const parsedContext = parseFileBrowseContext(rawContext);
  if (!parsedContext) {
    return { status: 'stale', keysToClear: STALE_BROWSE_KEYS };
  }

  const selectedProject = projects.find((project) => project.id === selectedProjectId);
  if (!selectedProject) {
    return { status: 'stale', keysToClear: STALE_BROWSE_KEYS };
  }

  if (parsedContext.type === 'change') {
    const hasChange = selectedProject.changes.some((change) => change.id === parsedContext.changeId);
    if (!hasChange) {
      return { status: 'stale', keysToClear: STALE_BROWSE_KEYS };
    }

    return {
      status: 'restored',
      context: parsedContext,
      tabs: {
        desktopCenterTab: 'changes',
        desktopRightTab: 'files',
        mobileActiveTab: 'files',
      },
    };
  }

  const worktrees = worktreesByProjectId?.[selectedProjectId];
  if (!worktrees) {
    return { status: 'defer' };
  }

  const hasWorktree = worktrees.some((worktree) => worktree.branch === parsedContext.worktreeBranch);
  if (!hasWorktree) {
    return { status: 'stale', keysToClear: STALE_BROWSE_KEYS };
  }

  return {
    status: 'restored',
    context: parsedContext,
    tabs: {
      desktopCenterTab: 'worktrees',
      desktopRightTab: 'files',
      mobileActiveTab: 'files',
    },
  };
}
