import React, { useEffect, useRef } from 'react';
import { User, Bot } from 'lucide-react';
import { ProposalChatMessage } from '../api/types';
import { ToolCallIndicator } from './ToolCallIndicator';

interface ChatMessageListProps {
  messages: ProposalChatMessage[];
  /** Content currently being streamed, keyed by message_id */
  streamingContent: Record<string, string>;
}

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

export function ChatMessageList({ messages, streamingContent }: ChatMessageListProps) {
  const bottomRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom on new messages or streaming updates
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, streamingContent]);

  // Collect streaming message IDs that aren't finalized yet
  const streamingIds = Object.keys(streamingContent);

  return (
    <div className="flex-1 overflow-y-auto p-4 space-y-4">
      {messages.length === 0 && streamingIds.length === 0 && (
        <div className="flex flex-1 items-center justify-center py-16">
          <p className="text-sm text-[#52525b]">Start a conversation with the agent</p>
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

      <div ref={bottomRef} />
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
