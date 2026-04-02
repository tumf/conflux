import React, { useState, useCallback, useRef, useEffect } from 'react';
import { SendHorizontal } from 'lucide-react';

interface ChatInputProps {
  onSend: (content: string) => void;
  status?: 'ready' | 'submitted' | 'streaming' | 'recovering' | 'error';
  placeholder?: string;
}

export function ChatInput({ onSend, status = 'ready', placeholder = 'Type a message...' }: ChatInputProps) {
  const [value, setValue] = useState('');
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const disabled = status !== 'ready';

  const handleSubmit = useCallback(() => {
    const trimmed = value.trim();
    if (!trimmed || disabled) return;
    onSend(trimmed);
    setValue('');
  }, [value, disabled, onSend]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key !== 'Enter' || e.shiftKey) return;
      e.preventDefault();
      handleSubmit();
    },
    [handleSubmit],
  );

  // Auto-resize textarea
  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = 'auto';
    el.style.height = `${Math.min(el.scrollHeight, 160)}px`;
  }, [value]);

  return (
    <div className="flex items-end gap-2 border-t border-border p-3">
      <textarea
        ref={textareaRef}
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder={placeholder}
        rows={1}
        className="min-h-[2.25rem] flex-1 resize-none rounded-md border border-border bg-surface px-3 py-2 text-sm text-text placeholder:text-text-subtle focus:border-accent focus:outline-none"
      />
      <button
        onClick={handleSubmit}
        disabled={disabled || !value.trim()}
        className="flex size-9 shrink-0 items-center justify-center rounded-md bg-accent text-white transition-colors hover:bg-accent-hover disabled:opacity-50 disabled:hover:bg-accent"
        aria-label="Send message"
      >
        <SendHorizontal className="size-4" />
      </button>
    </div>
  );
}
