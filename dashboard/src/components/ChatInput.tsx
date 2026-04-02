import React, { useState, useCallback, useRef, useEffect } from 'react';
import { SendHorizontal } from 'lucide-react';

interface ChatInputProps {
  onSend: (content: string) => void;
  isSubmissionLocked?: boolean;
  placeholder?: string;
  clearVersion?: number;
}

export function ChatInput({
  onSend,
  isSubmissionLocked = false,
  placeholder = 'Type a message...',
  clearVersion = 0,
}: ChatInputProps) {
  const [value, setValue] = useState('');
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const prevClearVersionRef = useRef(clearVersion);

  const handleSubmit = useCallback(() => {
    const trimmed = value.trim();
    if (!trimmed || isSubmissionLocked) return;
    onSend(trimmed);
  }, [value, isSubmissionLocked, onSend]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key !== 'Enter' || e.shiftKey) return;
      e.preventDefault();
      handleSubmit();
    },
    [handleSubmit],
  );

  useEffect(() => {
    if (clearVersion === prevClearVersionRef.current) return;
    prevClearVersionRef.current = clearVersion;
    setValue('');
  }, [clearVersion]);

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
        disabled={isSubmissionLocked}
        className="min-h-[2.25rem] flex-1 resize-none rounded-md border border-border bg-surface px-3 py-2 text-sm text-text placeholder:text-text-subtle focus:border-accent focus:outline-none disabled:opacity-70"
      />
      <button
        onClick={handleSubmit}
        disabled={isSubmissionLocked || !value.trim()}
        className="flex size-9 shrink-0 items-center justify-center rounded-md bg-accent text-white transition-colors hover:bg-accent-hover disabled:opacity-50 disabled:hover:bg-accent"
        aria-label="Send message"
      >
        <SendHorizontal className="size-4" />
      </button>
    </div>
  );
}
