import React, { useMemo } from 'react';
import { AnsiUp } from 'ansi-up';
import { RemoteLogEntry } from '../api/types';

interface LogEntryProps {
  entry: RemoteLogEntry;
  showProjectLabel?: boolean;
}

const levelConfig: Record<string, { label: string; color: string; bg: string }> = {
  info: { label: 'INFO', color: 'text-[#3b82f6]', bg: '' },
  warn: { label: 'WARN', color: 'text-[#f59e0b]', bg: 'bg-[#451a03]/20' },
  error: { label: 'ERR ', color: 'text-[#ef4444]', bg: 'bg-[#450a0a]/20' },
};

// Module-level singleton: AnsiUp is stateful (tracks incomplete sequences
// across calls), but we create one instance per component render via useMemo
// to avoid cross-entry state leakage.

/**
 * Convert ANSI escape sequences to safe HTML.
 *
 * Uses ansi-up for SGR color codes and adds lightweight handling for
 * bold (SGR 1) and underline (SGR 4) which ansi-up maps to bright
 * colors only.
 */
export function ansiToHtml(raw: string): string {
  const ansiUp = new AnsiUp();
  ansiUp.useClasses = true;
  // escapeForHtml is true by default – HTML special chars are escaped
  // before ANSI processing, preventing XSS.

  // --- Pre-process: extract bold/underline state ---
  // ansi-up treats SGR 1 as "bright" (color shift) and ignores SGR 4.
  // We strip these codes before ansi-up and re-apply them as wrapper
  // spans afterwards.  Because ansi-up escapes HTML first, the Unicode
  // PUA markers we inject cannot collide with user content and will
  // survive the conversion unchanged.
  const BOLD_OPEN = '\uE000';
  const BOLD_CLOSE = '\uE001';
  const UNDERLINE_OPEN = '\uE002';
  const UNDERLINE_CLOSE = '\uE003';

  let text = raw;

  // Replace SGR 1 (bold on) / 22 (bold off) with markers
  text = text.replace(/\x1b\[1m/g, BOLD_OPEN);
  text = text.replace(/\x1b\[22m/g, BOLD_CLOSE);

  // Replace SGR 4 (underline on) / 24 (underline off) with markers
  text = text.replace(/\x1b\[4m/g, UNDERLINE_OPEN);
  text = text.replace(/\x1b\[24m/g, UNDERLINE_CLOSE);

  // SGR 0 (reset all) should also close bold/underline.
  // We insert close markers before every reset so open tags get closed.
  text = text.replace(/\x1b\[0m/g, `${BOLD_CLOSE}${UNDERLINE_CLOSE}\x1b[0m`);

  // --- ansi-up conversion ---
  let html = ansiUp.ansi_to_html(text);

  // --- Post-process: replace markers with styled spans ---
  html = html
    .replace(new RegExp(BOLD_OPEN, 'g'), '<span class="ansi-bold">')
    .replace(new RegExp(BOLD_CLOSE, 'g'), '</span>')
    .replace(new RegExp(UNDERLINE_OPEN, 'g'), '<span class="ansi-underline">')
    .replace(new RegExp(UNDERLINE_CLOSE, 'g'), '</span>');

  return html;
}

export function LogEntry({ entry, showProjectLabel = false }: LogEntryProps) {
  const date = new Date(entry.timestamp);
  const timeStr = date.toLocaleTimeString('en', { hour12: false });
  const cfg = levelConfig[entry.level] ?? levelConfig.info;

  const messageHtml = useMemo(() => ansiToHtml(entry.message), [entry.message]);

  return (
    <div className={`flex gap-2 rounded px-2 py-1 font-mono text-xs ${cfg.bg}`}>
      <span className="shrink-0 text-[#3f3f46]">{timeStr}</span>
      <span className={`shrink-0 ${cfg.color}`}>{cfg.label}</span>
      {showProjectLabel && entry.project_id ? (
        <span className="shrink-0 rounded bg-[#18181b] px-1.5 py-0.5 text-[#71717a]">
          {entry.project_id}
        </span>
      ) : null}
      <span
        className="text-[#a1a1aa] break-all ansi-log-message"
        dangerouslySetInnerHTML={{ __html: messageHtml }}
      />
    </div>
  );
}
