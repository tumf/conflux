import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Bot, Copy, User } from 'lucide-react';

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

function copyTextToClipboard(text: string): void {
  if (typeof navigator === 'undefined' || !navigator.clipboard?.writeText) {
    return;
  }
  void navigator.clipboard.writeText(text);
}

function formatRelativeTime(timestamp: string): string {
  const target = new Date(timestamp).getTime();
  if (Number.isNaN(target)) {
    return 'time unavailable';
  }

  const diffSeconds = Math.round((target - Date.now()) / 1000);
  const absSeconds = Math.abs(diffSeconds);
  const rtf = new Intl.RelativeTimeFormat('en', { numeric: 'auto' });

  if (absSeconds < 60) {
    return rtf.format(diffSeconds, 'second');
  }

  const diffMinutes = Math.round(diffSeconds / 60);
  if (Math.abs(diffMinutes) < 60) {
    return rtf.format(diffMinutes, 'minute');
  }

  const diffHours = Math.round(diffSeconds / 3600);
  if (Math.abs(diffHours) < 24) {
    return rtf.format(diffHours, 'hour');
  }

  const diffDays = Math.round(diffSeconds / 86400);
  return rtf.format(diffDays, 'day');
}

function renderCodeBlock(code: string, language: string | null, key: string): React.ReactNode {
  return (
    <div key={key} className="my-2 overflow-hidden rounded border border-[#27272a] bg-[#18181b]">
      <div className="flex items-center justify-between border-b border-[#27272a] px-2 py-1 text-[11px] text-[#a1a1aa]">
        <span>{language ?? 'code'}</span>
        <button
          type="button"
          onClick={() => copyTextToClipboard(code)}
          className="inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[#d4d4d8] transition hover:bg-[#27272a]"
          title="Copy code"
          aria-label="Copy code"
        >
          <Copy className="size-3" />
          <span>Copy</span>
        </button>
      </div>
      <pre className="overflow-x-auto p-2 font-mono text-xs text-[#d4d4d8]">
        <code>{code}</code>
      </pre>
    </div>
  );
}

function renderInlineMarkdown(text: string): React.ReactNode {
  const parts: React.ReactNode[] = [];
  let remaining = text;
  let key = 0;

  while (remaining.length > 0) {
    const codeMatch = remaining.match(/^`([^`]+)`/);
    if (codeMatch) {
      parts.push(
        <code key={key++} className="rounded bg-border px-1 py-0.5 font-mono text-[0.85em] text-accent">
          {codeMatch[1]}
        </code>,
      );
      remaining = remaining.slice(codeMatch[0].length);
      continue;
    }

    const boldMatch = remaining.match(/^\*\*([^*]+)\*\*/);
    if (boldMatch) {
      parts.push(<strong key={key++}>{boldMatch[1]}</strong>);
      remaining = remaining.slice(boldMatch[0].length);
      continue;
    }

    const linkMatch = remaining.match(/^\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)/);
    if (linkMatch) {
      parts.push(
        <a
          key={key++}
          href={linkMatch[2]}
          target="_blank"
          rel="noopener noreferrer"
          className="text-[#818cf8] underline underline-offset-2 hover:text-[#a5b4fc]"
        >
          {linkMatch[1]}
        </a>,
      );
      remaining = remaining.slice(linkMatch[0].length);
      continue;
    }

    const nextSpecial = remaining.search(/[`*\[]/);
    if (nextSpecial === -1) {
      parts.push(remaining);
      break;
    }

    if (nextSpecial === 0) {
      parts.push(remaining[0]);
      remaining = remaining.slice(1);
      continue;
    }

    parts.push(remaining.slice(0, nextSpecial));
    remaining = remaining.slice(nextSpecial);
  }

  return parts.length === 1 ? parts[0] : <>{parts}</>;
}

function renderMarkdownSimple(content: string): React.ReactNode {
  const lines = content.split('\n');
  const elements: React.ReactNode[] = [];

  let inCodeBlock = false;
  let codeLines: string[] = [];
  let codeLanguage: string | null = null;
  let codeKey = 0;

  let currentUlItems: string[] = [];
  let currentOlItems: string[] = [];

  const flushUnorderedList = () => {
    if (currentUlItems.length === 0) {
      return;
    }
    elements.push(
      <ul key={`ul-${elements.length}`} className="my-1 list-disc space-y-1 pl-6">
        {currentUlItems.map((item, idx) => (
          <li key={`ul-item-${idx}`} className="break-words">
            {renderInlineMarkdown(item)}
          </li>
        ))}
      </ul>,
    );
    currentUlItems = [];
  };

  const flushOrderedList = () => {
    if (currentOlItems.length === 0) {
      return;
    }
    elements.push(
      <ol key={`ol-${elements.length}`} className="my-1 list-decimal space-y-1 pl-6">
        {currentOlItems.map((item, idx) => (
          <li key={`ol-item-${idx}`} className="break-words">
            {renderInlineMarkdown(item)}
          </li>
        ))}
      </ol>,
    );
    currentOlItems = [];
  };

  const flushCodeBlock = () => {
    if (!inCodeBlock) {
      return;
    }
    elements.push(renderCodeBlock(codeLines.join('\n'), codeLanguage, `code-${codeKey++}`));
    inCodeBlock = false;
    codeLines = [];
    codeLanguage = null;
  };

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];

    if (line.startsWith('```')) {
      flushUnorderedList();
      flushOrderedList();

      if (inCodeBlock) {
        flushCodeBlock();
      } else {
        inCodeBlock = true;
        codeLanguage = line.slice(3).trim() || null;
      }
      continue;
    }

    if (inCodeBlock) {
      codeLines.push(line);
      continue;
    }

    const headingMatch = line.match(/^(#{1,3})\s+(.+)$/);
    if (headingMatch) {
      flushUnorderedList();
      flushOrderedList();

      const level = headingMatch[1].length;
      const text = headingMatch[2];
      if (level === 1) {
        elements.push(
          <h1 key={`line-${i}`} className="mt-2 text-xl font-semibold text-white">
            {renderInlineMarkdown(text)}
          </h1>,
        );
      } else if (level === 2) {
        elements.push(
          <h2 key={`line-${i}`} className="mt-2 text-lg font-semibold text-white">
            {renderInlineMarkdown(text)}
          </h2>,
        );
      } else {
        elements.push(
          <h3 key={`line-${i}`} className="mt-2 text-base font-semibold text-[#e4e4e7]">
            {renderInlineMarkdown(text)}
          </h3>,
        );
      }
      continue;
    }

    const unorderedMatch = line.match(/^\s*[-*]\s+(.+)$/);
    if (unorderedMatch) {
      flushOrderedList();
      currentUlItems.push(unorderedMatch[1]);
      continue;
    }

    const orderedMatch = line.match(/^\s*\d+\.\s+(.+)$/);
    if (orderedMatch) {
      flushUnorderedList();
      currentOlItems.push(orderedMatch[1]);
      continue;
    }

    if (/^\s*---\s*$/.test(line)) {
      flushUnorderedList();
      flushOrderedList();
      elements.push(<hr key={`line-${i}`} className="my-3 border-[#3f3f46]" />);
      continue;
    }

    flushUnorderedList();
    flushOrderedList();

    elements.push(
      <p key={`line-${i}`} className="min-h-[1.25em] whitespace-pre-wrap break-words">
        {renderInlineMarkdown(line)}
      </p>,
    );
  }

  flushCodeBlock();
  flushUnorderedList();
  flushOrderedList();

  return elements;
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
    if (!scroller) {
      return;
    }

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
        className="h-full flex-1 space-y-4 overflow-y-auto p-4"
        data-testid="chat-scroll-container"
        onScroll={updateNearBottom}
      >
        {messages.length === 0 && streamingIds.length === 0 && (
          <div className="flex flex-1 items-center justify-center py-16">
            <div className="flex max-w-md flex-col items-center gap-3 text-center">
              <div className="flex size-10 items-center justify-center rounded-full bg-accent/20">
                <Bot className="size-5 text-accent" />
              </div>
              <p className="text-sm font-medium text-text">Start a conversation with the agent</p>
              <p className="text-xs text-text-subtle">Try one of these prompts to get started:</p>
              <div className="flex flex-wrap items-center justify-center gap-2">
                {EXAMPLE_PROMPTS.map((prompt) => (
                  <button
                    key={prompt}
                    type="button"
                    className="rounded-full border border-border bg-surface px-3 py-1 text-xs text-text-muted transition-colors hover:border-border-hover hover:text-text"
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
    <div className={`group flex items-start gap-3 ${isUser ? 'flex-row-reverse' : ''}`}>
      <div
        className={`flex size-7 shrink-0 items-center justify-center rounded-full ${
          isUser ? 'bg-border' : 'bg-accent/20'
        }`}
      >
        {isUser ? <User className="size-4 text-text-muted" /> : <Bot className="size-4 text-accent" />}
      </div>
      <div
        className={`relative min-w-0 max-w-[80%] space-y-2 rounded-lg px-3 py-2 text-sm ${
          isUser ? 'bg-accent/20 text-text' : 'bg-surface-2 text-text'
        }`}
      >
        {!isUser && (
          <button
            type="button"
            onClick={() => copyTextToClipboard(message.content)}
            className="absolute right-2 top-2 inline-flex items-center gap-1 rounded bg-[#27272a]/90 px-1.5 py-1 text-xs text-[#d4d4d8] opacity-0 transition hover:bg-[#3f3f46] group-hover:opacity-100"
            title="Copy message"
            aria-label="Copy message"
          >
            <Copy className="size-3" />
            <span>Copy</span>
          </button>
        )}

        <div>{renderMarkdownSimple(message.content)}</div>

        <p className="text-[11px] text-[#71717a] opacity-0 transition group-hover:opacity-100" title={message.timestamp}>
          {formatRelativeTime(message.timestamp)}
        </p>

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
