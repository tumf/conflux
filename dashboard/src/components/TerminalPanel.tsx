import React, { useState, useCallback, useRef, useEffect } from 'react';
import { Plus, X, Terminal as TerminalIcon, ChevronUp, ChevronDown } from 'lucide-react';
import { TerminalTab } from './TerminalTab';
import {
  createTerminalSession,
  deleteTerminalSession,
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

export function TerminalPanel({ projectId, root, isExpanded, onToggleExpand }: TerminalPanelProps) {
  const [tabs, setTabs] = useState<TabInfo[]>([]);
  const [activeTabId, setActiveTabId] = useState<string | null>(null);
  const [isCreating, setIsCreating] = useState(false);
  const tabsRef = useRef(tabs);
  tabsRef.current = tabs;

  // Create the first terminal session when panel is first expanded
  const hasAutoCreated = useRef(false);
  useEffect(() => {
    if (isExpanded && tabs.length === 0 && !hasAutoCreated.current && !isCreating) {
      hasAutoCreated.current = true;
      handleCreateTab();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isExpanded]);

  // Reset auto-create flag when context changes
  useEffect(() => {
    hasAutoCreated.current = false;
  }, [projectId, root]);

  const handleCreateTab = useCallback(async () => {
    if (isCreating) return;
    setIsCreating(true);
    try {
      const session = await createTerminalSession({ project_id: projectId, root, rows: 24, cols: 80 });
      const newTab: TabInfo = { session, attached: false };
      setTabs((prev) => [...prev, newTab]);
      setActiveTabId(session.id);
    } catch (err) {
      console.error('Failed to create terminal session:', err);
    } finally {
      setIsCreating(false);
    }
  }, [cwd, isCreating]);

  const handleCloseTab = useCallback(
    async (sessionId: string) => {
      try {
        await deleteTerminalSession(sessionId);
      } catch (err) {
        console.error('Failed to delete terminal session:', err);
      }
      setTabs((prev) => prev.filter((t) => t.session.id !== sessionId));
      setActiveTabId((prev) => {
        if (prev === sessionId) {
          const remaining = tabsRef.current.filter((t) => t.session.id !== sessionId);
          return remaining.length > 0 ? remaining[remaining.length - 1].session.id : null;
        }
        return prev;
      });
    },
    [],
  );

  const handleSelectTab = useCallback((sessionId: string) => {
    setActiveTabId(sessionId);
  }, []);

  const activeTab = tabs.find((t) => t.session.id === activeTabId);

  return (
    <div className="flex flex-col border-t border-[#27272a]">
      {/* Toggle bar */}
      <button
        onClick={onToggleExpand}
        className="flex items-center gap-2 px-3 py-1.5 text-xs text-[#a1a1aa] transition-colors hover:bg-[#27272a]/50"
      >
        <TerminalIcon className="size-3.5" />
        <span>Terminal</span>
        {tabs.length > 0 && (
          <span className="rounded bg-[#27272a] px-1.5 py-0.5 text-[10px] text-[#71717a]">
            {tabs.length}
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
              {tabs.map((tab) => (
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
                      {tab.session.id.slice(0, 12)}
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
            {tabs.length === 0 && (
              <div className="flex h-full items-center justify-center">
                <p className="text-xs text-[#52525b]">No terminal sessions</p>
              </div>
            )}
            {tabs.map((tab) => (
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
