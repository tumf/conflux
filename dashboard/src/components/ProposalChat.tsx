import React, { useCallback, useMemo } from 'react';
import { ArrowLeft } from 'lucide-react';
import { ProposalSession, ProposalChatMessage, ElicitationRequest, ToolCallInfo, ToolCallStatus } from '../api/types';
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
  onStreamingChunk: (messageId: string, content: string) => void;
  onToolCallStart: (sessionId: string, messageId: string, toolCall: ToolCallInfo) => void;
  onToolCallUpdate: (sessionId: string, messageId: string, toolCallId: string, status: ToolCallStatus) => void;
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
  onStreamingChunk,
  onToolCallStart,
  onToolCallUpdate,
  onElicitation,
  onClickChange,
  isLoading = false,
}: ProposalChatProps) {
  const agentMessageId = `agent-${session.id}`;
  const { sendPrompt, sendElicitationResponse, sendCancel, status } = useProposalWebSocket({
    projectId,
    sessionId: session.id,
    onMessageChunk: useCallback(
      (content: string) => {
        onStreamingChunk(agentMessageId, content);
      },
      [agentMessageId, onStreamingChunk],
    ),
    onToolCall: useCallback(
      (toolCall: ToolCallInfo) => {
        onToolCallStart(session.id, agentMessageId, toolCall);
      },
      [agentMessageId, session.id, onToolCallStart],
    ),
    onToolCallUpdate: useCallback(
      (toolCallId: string, toolCallStatus: ToolCallStatus) => {
        onToolCallUpdate(session.id, agentMessageId, toolCallId, toolCallStatus);
      },
      [agentMessageId, session.id, onToolCallUpdate],
    ),
    onElicitationRequest: useCallback(
      (elicitation: ElicitationRequest) => {
        onElicitation(elicitation);
      },
      [onElicitation],
    ),
    onTurnComplete: useCallback(
      () => {
        const streamed = streamingContent[agentMessageId] ?? '';
        const existingMessage = messages.find((message) => message.id === agentMessageId);
        const assistantMessage: ProposalChatMessage = {
          id: agentMessageId,
          role: 'assistant',
          content: streamed || existingMessage?.content || '',
          timestamp: new Date().toISOString(),
          tool_calls: existingMessage?.tool_calls,
        };
        onAppendMessage(session.id, assistantMessage);
      },
      [agentMessageId, messages, onAppendMessage, session.id, streamingContent],
    ),
    onError: useCallback(
      (message: string) => {
        console.error('Proposal WS error:', message);
      },
      [],
    ),
  });

  const handleSend = useCallback(
    (content: string) => {
      // Create a user message and append it locally
      const userMsg: ProposalChatMessage = {
        id: `user-${Date.now()}`,
        role: 'user',
        content,
        timestamp: new Date().toISOString(),
      };
      onAppendMessage(session.id, userMsg);
      sendPrompt(content);
    },
    [session.id, onAppendMessage, sendPrompt],
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
                    : 'Type a message... (Ctrl+Enter to send)'
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
