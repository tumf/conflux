import React, { useState, useCallback, useRef, useEffect } from 'react';
import { Plus, X, Terminal as TerminalIcon, ChevronUp, ChevronDown } from 'lucide-react';
import { TerminalTab } from './TerminalTab';
import {
  createTerminalSession,
  deleteTerminalSession,
  listTerminalSessions,
  TerminalSessionInfo,
} from '../api/restClient';

interface TerminalPanelProps {
  /** Project ID for terminal session creation */
  projectId: string;
  /** Root parameter matching file browser context */
  root: string;
  /** Whether the panel is expanded */
  isExpanded: boolean;
  /** Callback to toggle expansion */
  onToggleExpand: () => void;
}

interface TabInfo {
  session: TerminalSessionInfo;
  /** Whether this tab has been attached (WebSocket opened) */
  attached: boolean;
}

/**
 * Extract a display label from a root string.
 * "worktree:feature-x" → "feature-x", "base" → "base"
 */
function rootToLabel(root: string): string {
  if (root.startsWith('worktree:')) {
    return root.slice('worktree:'.length);
  }
  return root || 'terminal';
}

export function TerminalPanel({ projectId, root, isExpanded, onToggleExpand }: TerminalPanelProps) {
  // allTabs stores every session we know about (across all roots).
  // We filter for display based on current root.
  const [allTabs, setAllTabs] = useState<TabInfo[]>([]);
  const [activeTabId, setActiveTabId] = useState<string | null>(null);
  const [isCreating, setIsCreating] = useState(false);
  const [hasRestoredSessions, setHasRestoredSessions] = useState(false);
  const allTabsRef = useRef(allTabs);
  allTabsRef.current = allTabs;

  // Tabs visible for the current root context
  const visibleTabs = allTabs.filter(
    (t) => t.session.project_id === projectId && t.session.root === root,
  );

  // Count of all sessions (across all roots) for the badge
  const totalCount = allTabs.length;

  // Restore existing sessions on mount
  useEffect(() => {
    let cancelled = false;
    async function restoreSessions() {
      try {
        const sessions = await listTerminalSessions();
        if (cancelled) return;
        if (sessions.length > 0) {
          const restoredTabs: TabInfo[] = sessions.map((session) => ({
            session,
            attached: false,
          }));
          setAllTabs(restoredTabs);

          // Set active tab to the first matching session for current context
          const matching = sessions.filter(
            (s) => s.project_id === projectId && s.root === root,
          );
          if (matching.length > 0) {
            setActiveTabId(matching[0].id);
          }
        }
      } catch (err) {
        console.error('Failed to restore terminal sessions:', err);
      } finally {
        if (!cancelled) {
          setHasRestoredSessions(true);
        }
      }
    }
    restoreSessions();
    return () => {
      cancelled = true;
    };
    // Only run once on mount
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Auto-create a terminal session when panel expands and no matching session exists
  const hasAutoCreatedForContext = useRef<string | null>(null);
  useEffect(() => {
    if (!hasRestoredSessions) return;
    const contextKey = `${projectId}:${root}`;
    if (isExpanded && visibleTabs.length === 0 && hasAutoCreatedForContext.current !== contextKey && !isCreating) {
      hasAutoCreatedForContext.current = contextKey;
      handleCreateTab();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isExpanded, hasRestoredSessions, visibleTabs.length, projectId, root]);

  // When root changes, switch active tab to a matching session if available
  useEffect(() => {
    const matching = allTabsRef.current.filter(
      (t) => t.session.project_id === projectId && t.session.root === root,
    );
    if (matching.length > 0) {
      // If active tab is already matching, keep it; otherwise switch to first match
      const currentlyActive = matching.find((t) => t.session.id === activeTabId);
      if (!currentlyActive) {
        setActiveTabId(matching[0].session.id);
      }
    } else {
      setActiveTabId(null);
    }
  }, [projectId, root, activeTabId]);

  const handleCreateTab = useCallback(async () => {
    if (isCreating) return;
    setIsCreating(true);
    try {
      const session = await createTerminalSession({ project_id: projectId, root, rows: 24, cols: 80 });
      const newTab: TabInfo = { session, attached: false };
      setAllTabs((prev) => [...prev, newTab]);
      setActiveTabId(session.id);
    } catch (err) {
      console.error('Failed to create terminal session:', err);
    } finally {
      setIsCreating(false);
    }
  }, [projectId, root, isCreating]);

  const handleCloseTab = useCallback(
    async (sessionId: string) => {
      try {
        await deleteTerminalSession(sessionId);
      } catch (err) {
        console.error('Failed to delete terminal session:', err);
      }
      setAllTabs((prev) => prev.filter((t) => t.session.id !== sessionId));
      setActiveTabId((prev) => {
        if (prev === sessionId) {
          const remaining = allTabsRef.current.filter(
            (t) => t.session.id !== sessionId && t.session.project_id === projectId && t.session.root === root,
          );
          return remaining.length > 0 ? remaining[remaining.length - 1].session.id : null;
        }
        return prev;
      });
    },
    [projectId, root],
  );

  const handleSelectTab = useCallback((sessionId: string) => {
    setActiveTabId(sessionId);
  }, []);

  return (
    <div className="flex flex-col border-t border-[#27272a]">
      {/* Toggle bar */}
      <button
        onClick={onToggleExpand}
        className="flex items-center gap-2 px-3 py-1.5 text-xs text-[#a1a1aa] transition-colors hover:bg-[#27272a]/50"
      >
        <TerminalIcon className="size-3.5" />
        <span>Terminal</span>
        {totalCount > 0 && (
          <span className="rounded bg-[#27272a] px-1.5 py-0.5 text-[10px] text-[#71717a]">
            {totalCount}
          </span>
        )}
        <span className="flex-1" />
        {isExpanded ? (
          <ChevronDown className="size-3.5" />
        ) : (
          <ChevronUp className="size-3.5" />
        )}
      </button>

      {/* Terminal content (only rendered when expanded) */}
      {isExpanded && (
        <div className="flex flex-col" style={{ height: '300px' }}>
          {/* Tab bar */}
          <div className="flex items-center border-b border-[#27272a] bg-[#0a0a0a]">
            <div className="flex flex-1 items-center overflow-x-auto">
              {visibleTabs.map((tab) => (
                <div
                  key={tab.session.id}
                  className={`group flex items-center gap-1 border-r border-[#27272a] px-2 py-1 text-xs ${
                    activeTabId === tab.session.id
                      ? 'bg-[#18181b] text-[#e4e4e7]'
                      : 'text-[#71717a] hover:bg-[#18181b]/50'
                  }`}
                >
                  <button
                    onClick={() => handleSelectTab(tab.session.id)}
                    className="flex items-center gap-1"
                  >
                    <TerminalIcon className="size-3" />
                    <span className="max-w-[120px] truncate">
                      {rootToLabel(tab.session.root)}
                    </span>
                  </button>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      handleCloseTab(tab.session.id);
                    }}
                    className="rounded p-0.5 opacity-0 transition-opacity hover:bg-[#27272a] group-hover:opacity-100"
                    title="Close terminal"
                  >
                    <X className="size-3" />
                  </button>
                </div>
              ))}
            </div>
            <button
              onClick={handleCreateTab}
              disabled={isCreating}
              className="p-1.5 text-[#71717a] transition-colors hover:bg-[#27272a] hover:text-[#a1a1aa] disabled:opacity-50"
              title="New terminal"
            >
              <Plus className="size-3.5" />
            </button>
          </div>

          {/* Terminal content area */}
          <div className="relative flex-1 overflow-hidden bg-[#0a0a0a]">
            {visibleTabs.length === 0 && (
              <div className="flex h-full items-center justify-center">
                <p className="text-xs text-[#52525b]">No terminal sessions</p>
              </div>
            )}
            {/* Render all tabs but only show the active one.
                Tabs for other roots are kept alive but hidden. */}
            {allTabs.map((tab) => (
              <div
                key={tab.session.id}
                className="absolute inset-0"
                style={{ display: activeTabId === tab.session.id ? 'block' : 'none' }}
              >
                <TerminalTab
                  sessionId={tab.session.id}
                  isActive={activeTabId === tab.session.id}
                />
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
