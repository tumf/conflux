import React, { useEffect, useMemo, useRef, useState } from 'react';
import { User, Bot } from 'lucide-react';
import { ProposalChatMessage } from '../api/types';
import { ToolCallIndicator } from './ToolCallIndicator';

interface ChatMessageListProps {
  messages: ProposalChatMessage[];
  /** Content currently being streamed, keyed by message_id */
  streamingContent: Record<string, string>;
  isAgentResponding?: boolean;
  onExamplePromptSelect?: (prompt: string) => void;
}

const AUTO_SCROLL_THRESHOLD_PX = 100;
const EXAMPLE_PROMPTS = [
  'Summarize the current proposal and open risks',
  'What implementation tasks should we tackle first?',
  'Draft acceptance criteria for this change',
];

function renderMarkdownSimple(content: string): React.ReactNode {
  // Simple markdown rendering: code blocks, bold, inline code
  // Full markdown library is Future Work
  const lines = content.split('\n');
  const elements: React.ReactNode[] = [];
  let inCodeBlock = false;
  let codeLines: string[] = [];
  let codeKey = 0;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (line.startsWith('```')) {
      if (inCodeBlock) {
        elements.push(
          <pre key={`code-${codeKey++}`} className="my-2 overflow-x-auto rounded bg-[#18181b] p-2 font-mono text-xs text-[#d4d4d8]">
            <code>{codeLines.join('\n')}</code>
          </pre>,
        );
        codeLines = [];
        inCodeBlock = false;
      } else {
        inCodeBlock = true;
      }
      continue;
    }
    if (inCodeBlock) {
      codeLines.push(line);
    } else {
      elements.push(
        <p key={`line-${i}`} className="min-h-[1.25em] whitespace-pre-wrap break-words">
          {renderInlineMarkdown(line)}
        </p>,
      );
    }
  }

  // Unclosed code block
  if (inCodeBlock && codeLines.length > 0) {
    elements.push(
      <pre key={`code-${codeKey}`} className="my-2 overflow-x-auto rounded bg-[#18181b] p-2 font-mono text-xs text-[#d4d4d8]">
        <code>{codeLines.join('\n')}</code>
      </pre>,
    );
  }

  return elements;
}

function renderInlineMarkdown(text: string): React.ReactNode {
  // Handle inline code and bold
  const parts: React.ReactNode[] = [];
  let remaining = text;
  let key = 0;

  while (remaining.length > 0) {
    // Inline code
    const codeMatch = remaining.match(/^`([^`]+)`/);
    if (codeMatch) {
      parts.push(
        <code key={key++} className="rounded bg-[#27272a] px-1 py-0.5 font-mono text-[0.85em] text-[#a5b4fc]">
          {codeMatch[1]}
        </code>,
      );
      remaining = remaining.slice(codeMatch[0].length);
      continue;
    }

    // Bold
    const boldMatch = remaining.match(/^\*\*([^*]+)\*\*/);
    if (boldMatch) {
      parts.push(<strong key={key++}>{boldMatch[1]}</strong>);
      remaining = remaining.slice(boldMatch[0].length);
      continue;
    }

    // Find next special character
    const nextSpecial = remaining.search(/[`*]/);
    if (nextSpecial === -1) {
      parts.push(remaining);
      break;
    }
    if (nextSpecial === 0) {
      // Not a matched pattern, consume the character
      parts.push(remaining[0]);
      remaining = remaining.slice(1);
    } else {
      parts.push(remaining.slice(0, nextSpecial));
      remaining = remaining.slice(nextSpecial);
    }
  }

  return parts.length === 1 ? parts[0] : <>{parts}</>;
}

function TypingIndicator() {
  return (
    <div className="flex items-start gap-3" data-testid="typing-indicator">
      <div className="flex size-7 shrink-0 items-center justify-center rounded-full bg-[#1e1b4b]">
        <Bot className="size-4 text-[#a5b4fc]" />
      </div>
      <div className="min-w-0 rounded-lg bg-[#18181b] px-3 py-2 text-sm text-[#d4d4d8]">
        <div className="flex items-center gap-2">
          <span className="text-[#a1a1aa]">Agent is thinking...</span>
          <span className="inline-flex gap-1" aria-hidden="true">
            <span className="size-1.5 animate-bounce rounded-full bg-[#6366f1] [animation-delay:-0.3s]" />
            <span className="size-1.5 animate-bounce rounded-full bg-[#6366f1] [animation-delay:-0.15s]" />
            <span className="size-1.5 animate-bounce rounded-full bg-[#6366f1]" />
          </span>
        </div>
      </div>
    </div>
  );
}

export function ChatMessageList({
  messages,
  streamingContent,
  isAgentResponding = false,
  onExamplePromptSelect,
}: ChatMessageListProps) {
  const scrollerRef = useRef<HTMLDivElement>(null);
  const bottomRef = useRef<HTMLDivElement>(null);
  const [isNearBottom, setIsNearBottom] = useState(true);
  const [hasUnreadMessages, setHasUnreadMessages] = useState(false);

  const streamingIds = useMemo(() => Object.keys(streamingContent), [streamingContent]);

  const updateNearBottom = () => {
    const scroller = scrollerRef.current;
    if (!scroller) return;
    const distanceToBottom = scroller.scrollHeight - (scroller.scrollTop + scroller.clientHeight);
    const nearBottom = distanceToBottom <= AUTO_SCROLL_THRESHOLD_PX;
    setIsNearBottom(nearBottom);
    if (nearBottom) {
      setHasUnreadMessages(false);
    }
  };

  useEffect(() => {
    updateNearBottom();
  }, []);

  useEffect(() => {
    if (isNearBottom) {
      bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
      return;
    }

    if (messages.length > 0 || streamingIds.length > 0) {
      setHasUnreadMessages(true);
    }
  }, [messages, streamingIds, isNearBottom]);

  const showTypingIndicator = isAgentResponding && streamingIds.length === 0;

  return (
    <div className="relative flex-1 overflow-hidden">
      <div
        ref={scrollerRef}
        className="flex-1 overflow-y-auto p-4 space-y-4 h-full"
        data-testid="chat-scroll-container"
        onScroll={updateNearBottom}
      >
        {messages.length === 0 && streamingIds.length === 0 && (
          <div className="flex flex-1 items-center justify-center py-16">
            <div className="flex max-w-md flex-col items-center gap-3 text-center">
              <div className="flex size-10 items-center justify-center rounded-full bg-[#1e1b4b]">
                <Bot className="size-5 text-[#a5b4fc]" />
              </div>
              <p className="text-sm font-medium text-[#d4d4d8]">Start a conversation with the agent</p>
              <p className="text-xs text-[#71717a]">Try one of these prompts to get started:</p>
              <div className="flex flex-wrap items-center justify-center gap-2">
                {EXAMPLE_PROMPTS.map((prompt) => (
                  <button
                    key={prompt}
                    type="button"
                    className="rounded-full border border-[#27272a] bg-[#111113] px-3 py-1 text-xs text-[#a1a1aa] transition-colors hover:border-[#3f3f46] hover:text-[#d4d4d8]"
                    onClick={() => onExamplePromptSelect?.(prompt)}
                  >
                    {prompt}
                  </button>
                ))}
              </div>
            </div>
          </div>
        )}

        {messages.map((msg) => (
          <MessageBubble key={msg.id} message={msg} />
        ))}

        {/* Render streaming messages that aren't finalized */}
        {streamingIds
          .filter((id) => !messages.some((m) => m.id === id))
          .map((id) => (
            <div key={`stream-${id}`} className="flex items-start gap-3">
              <div className="flex size-7 shrink-0 items-center justify-center rounded-full bg-[#1e1b4b]">
                <Bot className="size-4 text-[#a5b4fc]" />
              </div>
              <div className="min-w-0 flex-1 rounded-lg bg-[#18181b] px-3 py-2 text-sm text-[#d4d4d8]">
                {renderMarkdownSimple(streamingContent[id])}
                <span className="inline-block h-4 w-1 animate-pulse bg-[#6366f1] align-middle" />
              </div>
            </div>
          ))}

        {showTypingIndicator && <TypingIndicator />}

        <div ref={bottomRef} />
      </div>

      {hasUnreadMessages && (
        <div className="pointer-events-none absolute inset-x-0 bottom-3 flex justify-center">
          <button
            type="button"
            className="pointer-events-auto rounded-full border border-[#3f3f46] bg-[#18181b] px-3 py-1 text-xs font-medium text-[#e4e4e7] shadow-lg transition-colors hover:border-[#6366f1] hover:text-white"
            onClick={() => {
              bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
              setHasUnreadMessages(false);
            }}
          >
            ↓ New messages
          </button>
        </div>
      )}
    </div>
  );
}

function MessageBubble({ message }: { message: ProposalChatMessage }) {
  const isUser = message.role === 'user';

  return (
    <div className={`flex items-start gap-3 ${isUser ? 'flex-row-reverse' : ''}`}>
      <div
        className={`flex size-7 shrink-0 items-center justify-center rounded-full ${
          isUser ? 'bg-[#27272a]' : 'bg-[#1e1b4b]'
        }`}
      >
        {isUser ? (
          <User className="size-4 text-[#a1a1aa]" />
        ) : (
          <Bot className="size-4 text-[#a5b4fc]" />
        )}
      </div>
      <div
        className={`min-w-0 max-w-[80%] space-y-2 rounded-lg px-3 py-2 text-sm ${
          isUser
            ? 'bg-[#1e1b4b]/60 text-[#e0e7ff]'
            : 'bg-[#18181b] text-[#d4d4d8]'
        }`}
      >
        <div>{renderMarkdownSimple(message.content)}</div>

        {message.tool_calls && message.tool_calls.length > 0 && (
          <div className="flex flex-wrap gap-1.5 pt-1">
            {message.tool_calls.map((tc) => (
              <ToolCallIndicator key={tc.id} toolCall={tc} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
