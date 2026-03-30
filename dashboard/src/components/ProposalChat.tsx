import React, { useCallback, useRef } from 'react';
import { ArrowLeft } from 'lucide-react';
import { ElicitationRequest, ProposalChatMessage, ProposalSession, ToolCallInfo, ToolCallStatus } from '../api/types';
import { useProposalWebSocket } from '../hooks/useProposalWebSocket';
import { ChatMessageList } from './ChatMessageList';
import { ChatInput } from './ChatInput';
import { ElicitationDialog } from './ElicitationDialog';
import { ProposalChangesList } from './ProposalChangesList';
import { ProposalActions } from './ProposalActions';

interface ProposalChatProps {
  projectId: string;
  session: ProposalSession;
  messages: ProposalChatMessage[];
  streamingContent: Record<string, string>;
  activeElicitation: ElicitationRequest | null;
  isAgentResponding: boolean;
  onBack: () => void;
  onMerge: () => void;
  onClose: () => void;
  onAppendMessage: (sessionId: string, message: ProposalChatMessage) => void;
  onStartAssistantTurn: (sessionId: string, messageId: string, turnId?: string) => void;
  onStreamingChunk: (sessionId: string, messageId: string, content: string, turnId?: string) => void;
  onCompleteAssistantTurn: (sessionId: string, stopReason?: string) => void;
  onFailAssistantTurn: (sessionId: string, error: string) => void;
  onToolCallStart: (sessionId: string, messageId: string, toolCall: ToolCallInfo, turnId?: string) => void;
  onToolCallUpdate: (sessionId: string, messageId: string, toolCallId: string, status: ToolCallStatus, turnId?: string) => void;
  onElicitation: (elicitation: ElicitationRequest | null) => void;
  onClickChange?: (changeId: string) => void;
  isLoading?: boolean;
}

export function ProposalChat({
  projectId,
  session,
  messages,
  streamingContent,
  activeElicitation,
  isAgentResponding,
  onBack,
  onMerge,
  onClose,
  onAppendMessage,
  onStartAssistantTurn,
  onStreamingChunk,
  onCompleteAssistantTurn,
  onFailAssistantTurn,
  onToolCallStart,
  onToolCallUpdate,
  onElicitation,
  onClickChange,
  isLoading = false,
}: ProposalChatProps) {
  const pendingMessageIdRef = useRef<string | null>(null);

  const { sendPrompt, sendElicitationResponse, status } = useProposalWebSocket({
    projectId,
    sessionId: session.id,
    hasActiveTurn: () => pendingMessageIdRef.current !== null,
    onUserMessage: useCallback(
      (message: { id: string; content: string; timestamp: string }) => {
        onAppendMessage(session.id, {
          id: message.id,
          role: 'user',
          content: message.content,
          timestamp: message.timestamp,
          hydrated: true,
        });
      },
      [onAppendMessage, session.id],
    ),
    onMessageChunk: useCallback(
      (content: string, messageId?: string, turnId?: string) => {
        const resolvedMessageId = messageId ?? pendingMessageIdRef.current ?? `assistant-${session.id}-${Date.now()}`;
        if (!pendingMessageIdRef.current) {
          pendingMessageIdRef.current = resolvedMessageId;
          onStartAssistantTurn(session.id, resolvedMessageId, turnId);
        }
        onStreamingChunk(session.id, resolvedMessageId, content, turnId);
      },
      [onStartAssistantTurn, onStreamingChunk, session.id],
    ),
    onThoughtChunk: useCallback(
      (_content: string, _messageId?: string, _turnId?: string) => {
        // Intentionally ignored for now; thought chunks are separated at protocol level.
      },
      [],
    ),
    onToolCall: useCallback(
      (toolCall: ToolCallInfo, messageId?: string, turnId?: string) => {
        const resolvedMessageId = messageId ?? pendingMessageIdRef.current ?? `assistant-${session.id}-${Date.now()}`;
        if (!pendingMessageIdRef.current) {
          pendingMessageIdRef.current = resolvedMessageId;
          onStartAssistantTurn(session.id, resolvedMessageId, turnId);
        }
        onToolCallStart(session.id, resolvedMessageId, toolCall, turnId);
      },
      [onStartAssistantTurn, onToolCallStart, session.id],
    ),
    onToolCallUpdate: useCallback(
      (toolCallId: string, toolCallStatus: ToolCallStatus, messageId?: string, turnId?: string) => {
        const resolvedMessageId = messageId ?? pendingMessageIdRef.current;
        if (!resolvedMessageId) return;
        onToolCallUpdate(session.id, resolvedMessageId, toolCallId, toolCallStatus, turnId);
      },
      [onToolCallUpdate, session.id],
    ),
    onElicitationRequest: useCallback(
      (elicitation: ElicitationRequest) => {
        onElicitation(elicitation);
      },
      [onElicitation],
    ),
    onTurnComplete: useCallback(
      (stopReason: string, messageId?: string, turnId?: string) => {
        const resolvedMessageId = messageId ?? pendingMessageIdRef.current;
        if (resolvedMessageId) {
          onCompleteAssistantTurn(session.id, stopReason);
        }
        pendingMessageIdRef.current = null;
      },
      [onCompleteAssistantTurn, session.id],
    ),
    onError: useCallback(
      (message: string) => {
        console.error('Proposal WS error:', { message, sessionId: session.id });
        onFailAssistantTurn(session.id, message);
        pendingMessageIdRef.current = null;
      },
      [onFailAssistantTurn, session.id],
    ),
  });

  const handleSend = useCallback(
    (content: string) => {
      sendPrompt(content);
    },
    [sendPrompt],
  );

  const handleExamplePromptSelect = useCallback(
    (content: string) => {
      if (!content.trim()) return;
      sendPrompt(content);
    },
    [sendPrompt],
  );

  const handleElicitationSubmit = useCallback(
    (data: Record<string, unknown>) => {
      if (!activeElicitation) return;
      sendElicitationResponse(activeElicitation.id, 'accept', data);
      onElicitation(null);
    },
    [activeElicitation, sendElicitationResponse, onElicitation],
  );

  const handleElicitationDecline = useCallback(() => {
    if (!activeElicitation) return;
    sendElicitationResponse(activeElicitation.id, 'decline');
    onElicitation(null);
  }, [activeElicitation, sendElicitationResponse, onElicitation]);

  const handleElicitationCancel = useCallback(() => {
    if (!activeElicitation) return;
    sendElicitationResponse(activeElicitation.id, 'cancel');
    onElicitation(null);
  }, [activeElicitation, sendElicitationResponse, onElicitation]);

  const wsConnected = status === 'connected';

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-[#27272a] px-3 py-2">
        <div className="flex items-center gap-2">
          <button
            onClick={onBack}
            className="rounded p-1 text-[#52525b] transition-colors hover:text-[#a1a1aa]"
            aria-label="Back to project"
          >
            <ArrowLeft className="size-4" />
          </button>
          <div className="flex items-center gap-1.5">
            <span className="text-sm font-medium text-[#fafafa]">Proposal Session</span>
            <span className="rounded bg-[#27272a] px-1.5 py-0.5 font-mono text-xs text-[#71717a]">
              {session.worktree_branch}
            </span>
            <span
              className={`size-2 rounded-full ${
                wsConnected ? 'bg-[#22c55e]' : 'bg-[#52525b]'
              }`}
              title={wsConnected ? 'Connected' : 'Disconnected'}
            />
          </div>
        </div>
        <ProposalActions
          session={session}
          onMerge={onMerge}
          onClose={onClose}
          isLoading={isLoading}
        />
      </div>

      {/* Main content: chat + sidebar */}
      <div className="flex flex-1 overflow-hidden">
        {/* Chat area */}
        <div className="flex flex-1 flex-col overflow-hidden">
          <ChatMessageList
            messages={messages}
            streamingContent={streamingContent}
            isAgentResponding={isAgentResponding}
            onExamplePromptSelect={handleExamplePromptSelect}
          />
          <ChatInput
            onSend={handleSend}
            disabled={!wsConnected || isAgentResponding || !!activeElicitation}
            placeholder={
              !wsConnected
                ? 'Connecting...'
                : isAgentResponding
                  ? 'Agent is responding...'
                  : activeElicitation
                    ? 'Please respond to the agent request first'
                    : 'Type a message... (Enter to send, Shift+Enter for newline)'
            }
          />
        </div>

        {/* Changes sidebar */}
        <div className="hidden w-56 shrink-0 flex-col border-l border-[#27272a] md:flex">
          <ProposalChangesList
            projectId={projectId}
            sessionId={session.id}
            onClickChange={onClickChange}
          />
        </div>
      </div>

      {/* Elicitation dialog */}
      {activeElicitation && (
        <ElicitationDialog
          elicitation={activeElicitation}
          onSubmit={handleElicitationSubmit}
          onDecline={handleElicitationDecline}
          onCancel={handleElicitationCancel}
        />
      )}
    </div>
  );
}
