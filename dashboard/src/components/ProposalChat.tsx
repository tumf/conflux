import React, { useCallback, useMemo, useState } from 'react';
import { ArrowLeft, PanelRight } from 'lucide-react';

import { ProposalSession } from '../api/types';
import { useProposalChat } from '../hooks/useProposalChat';
import { ChatMessageList } from './ChatMessageList';
import { ChatInput } from './ChatInput';
import { ElicitationDialog } from './ElicitationDialog';
import { ProposalChangesList } from './ProposalChangesList';
import { ProposalActions } from './ProposalActions';
import { ChangesDrawer } from './ChangesDrawer';

interface ProposalChatProps {
  projectId: string;
  sessionId: string;
  onBack: () => void;
  onMerge: () => void;
  onClose: () => void;
  onClickChange?: (changeId: string) => void;
  isLoading?: boolean;
}

function statusPlaceholder(status: 'ready' | 'submitted' | 'streaming' | 'error', wsConnected: boolean): string {
  if (!wsConnected) {
    return 'Disconnected. Message will be queued and sent on reconnect.';
  }
  switch (status) {
    case 'submitted':
      return 'Message submitted. Waiting for agent response...';
    case 'streaming':
      return 'Agent is responding...';
    case 'error':
      return 'Last turn failed. Adjust your message and retry.';
    case 'ready':
    default:
      return 'Type a message... (Enter to send, Shift+Enter for newline)';
  }
}

export function ProposalChat({
  projectId,
  sessionId,
  onBack,
  onMerge,
  onClose,
  onClickChange,
  isLoading = false,
}: ProposalChatProps) {
  const [isChangesDrawerOpen, setIsChangesDrawerOpen] = useState(false);
  const {
    messages,
    status,
    sendMessage,
    stop,
    error,
    activeElicitation,
    sendElicitationResponse,
    wsConnected,
  } = useProposalChat(projectId, sessionId);

  const handleExamplePromptSelect = useCallback(
    (content: string) => {
      sendMessage(content);
    },
    [sendMessage],
  );

  const handleRetryMessage = useCallback(
    (messageId: string) => {
      const target = messages.find((message) => message.id === messageId && message.role === 'user');
      if (!target) {
        console.warn('Retry requested for missing user message', { sessionId, messageId });
        return;
      }
      sendMessage(target.content);
    },
    [messages, sendMessage, sessionId],
  );

  const activeSession: ProposalSession = useMemo(
    () => ({
      id: sessionId,
      project_id: projectId,
      status: 'active',
      worktree_branch: sessionId,
      is_dirty: false,
      uncommitted_files: [],
      created_at: '',
      updated_at: '',
    }),
    [projectId, sessionId],
  );

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      <div className="flex items-center justify-between border-b border-border px-3 py-2">
        <div className="flex items-center gap-2">
          <button
            onClick={onBack}
            className="rounded p-1 text-text-subtle transition-colors hover:text-text-muted"
            aria-label="Back to project"
          >
            <ArrowLeft className="size-4" />
          </button>
          <div className="flex items-center gap-1.5">
            <span className="text-sm font-medium text-text">Proposal Session</span>
            <span className="rounded bg-border px-1.5 py-0.5 font-mono text-xs text-text-muted">
              {sessionId}
            </span>
            <span
              className={`size-2 rounded-full ${wsConnected ? 'bg-success' : 'bg-text-subtle'}`}
              title={wsConnected ? 'Connected' : 'Disconnected'}
            />
          </div>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            className="rounded p-1 text-[#52525b] transition-colors hover:text-[#a1a1aa] md:hidden"
            aria-label="Open changes drawer"
            onClick={() => {
              setIsChangesDrawerOpen(true);
            }}
          >
            <PanelRight className="size-4" />
          </button>
          <ProposalActions session={activeSession} onMerge={onMerge} onClose={onClose} isLoading={isLoading} />
        </div>
      </div>

      {error && (
        <div className="border-b border-red-900/60 bg-red-950/40 px-3 py-2 text-xs text-red-300">
          {error}
        </div>
      )}

      <div className="flex flex-1 overflow-hidden">
        <div className="flex flex-1 flex-col overflow-hidden">
          <ChatMessageList
            messages={messages}
            isAgentResponding={status === 'submitted' || status === 'streaming'}
            onExamplePromptSelect={handleExamplePromptSelect}
            onRetryMessage={handleRetryMessage}
          />
          <ChatInput
            onSend={sendMessage}
            status={status}
            placeholder={statusPlaceholder(status, wsConnected)}
          />
          {(status === 'submitted' || status === 'streaming') && (
            <div className="border-t border-border px-3 py-2 text-xs text-text-subtle">
              <button
                type="button"
                className="rounded border border-border px-2 py-1 text-text-muted hover:text-text"
                onClick={stop}
              >
                Stop generation
              </button>
            </div>
          )}
        </div>

        <div className="hidden w-56 shrink-0 flex-col border-l border-border md:flex">
          <ProposalChangesList
            projectId={projectId}
            sessionId={sessionId}
            onClickChange={onClickChange}
          />
        </div>
      </div>

      <ChangesDrawer
        isOpen={isChangesDrawerOpen}
        onClose={() => {
          setIsChangesDrawerOpen(false);
        }}
        title="Changes"
      >
        <ProposalChangesList
          projectId={projectId}
          sessionId={sessionId}
          onClickChange={(changeId) => {
            onClickChange?.(changeId);
            setIsChangesDrawerOpen(false);
          }}
        />
      </ChangesDrawer>

      {activeElicitation && (
        <ElicitationDialog
          elicitation={activeElicitation}
          onSubmit={(data) => sendElicitationResponse(activeElicitation.id, 'accept', data)}
          onDecline={() => sendElicitationResponse(activeElicitation.id, 'decline')}
          onCancel={() => sendElicitationResponse(activeElicitation.id, 'cancel')}
        />
      )}
    </div>
  );
}
