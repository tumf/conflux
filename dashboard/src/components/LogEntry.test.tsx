/**
 * @vitest-environment jsdom
 */

import React, { act } from 'react';
import { createRoot, Root } from 'react-dom/client';
import { afterEach, beforeAll, describe, expect, it } from 'vitest';
import { LogEntry, ansiToHtml } from './LogEntry';
import { RemoteLogEntry } from '../api/types';

// ---------------------------------------------------------------------------
// Unit tests for the `ansiToHtml` helper
// ---------------------------------------------------------------------------
describe('ansiToHtml', () => {
  it('converts ANSI color codes to span elements with CSS classes', () => {
    const html = ansiToHtml('\x1b[31mERROR\x1b[0m');
    expect(html).toContain('<span');
    expect(html).toContain('ansi-red-fg');
    expect(html).toContain('ERROR');
    // Raw escape codes must not be visible
    expect(html).not.toContain('[31m');
    expect(html).not.toContain('[0m');
  });

  it('returns plain text unchanged when no ANSI codes are present', () => {
    const html = ansiToHtml('hello world');
    expect(html).toBe('hello world');
  });

  it('escapes HTML special characters to prevent XSS', () => {
    const html = ansiToHtml('<script>alert("xss")</script>');
    expect(html).not.toContain('<script>');
    expect(html).toContain('&lt;script&gt;');
  });

  it('renders bold ANSI codes with ansi-bold class', () => {
    const html = ansiToHtml('\x1b[1mbold text\x1b[0m');
    expect(html).toContain('ansi-bold');
    expect(html).toContain('bold text');
  });

  it('renders underline ANSI codes with ansi-underline class', () => {
    const html = ansiToHtml('\x1b[4munderlined\x1b[0m');
    expect(html).toContain('ansi-underline');
    expect(html).toContain('underlined');
  });

  it('handles combined color and bold codes', () => {
    const html = ansiToHtml('\x1b[1m\x1b[32mbold green\x1b[0m');
    expect(html).toContain('ansi-bold');
    expect(html).toContain('bold green');
    // ansi-up maps bold+color to bright variant
    expect(html).toMatch(/ansi-(bright-)?green-fg/);
  });

  it('handles multiple ANSI segments in one message', () => {
    const html = ansiToHtml('\x1b[31mred\x1b[0m normal \x1b[34mblue\x1b[0m');
    expect(html).toContain('ansi-red-fg');
    expect(html).toContain('ansi-blue-fg');
    expect(html).toContain('normal');
  });
});

// ---------------------------------------------------------------------------
// Component rendering tests for LogEntry
// ---------------------------------------------------------------------------
let container: HTMLDivElement | null = null;
let root: Root | null = null;

beforeAll(() => {
  globalThis.IS_REACT_ACT_ENVIRONMENT = true;
});

function renderEntry(entry: RemoteLogEntry, showProjectLabel = false) {
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);

  act(() => {
    root!.render(
      <LogEntry entry={entry} showProjectLabel={showProjectLabel} />,
    );
  });

  return container;
}

afterEach(() => {
  if (root) {
    act(() => {
      root!.unmount();
    });
  }
  if (container) {
    container.remove();
  }
  root = null;
  container = null;
});

const baseEntry: RemoteLogEntry = {
  message: 'plain message',
  level: 'info',
  change_id: null,
  timestamp: '2026-03-29T12:00:00.000Z',
  project_id: null,
  operation: null,
  iteration: null,
};

describe('LogEntry component', () => {
  it('renders an ANSI-colored message as HTML spans', () => {
    const entry = { ...baseEntry, message: '\x1b[31mfail\x1b[0m' };
    const rendered = renderEntry(entry);
    const messageEl = rendered.querySelector('.ansi-log-message');
    expect(messageEl).not.toBeNull();
    expect(messageEl!.innerHTML).toContain('ansi-red-fg');
    expect(messageEl!.textContent).toContain('fail');
  });

  it('renders a plain message without extra markup', () => {
    const rendered = renderEntry(baseEntry);
    const messageEl = rendered.querySelector('.ansi-log-message');
    expect(messageEl).not.toBeNull();
    expect(messageEl!.textContent).toBe('plain message');
  });

  it('sanitizes script tags in messages', () => {
    const entry = { ...baseEntry, message: '<script>alert(1)</script>' };
    const rendered = renderEntry(entry);
    const messageEl = rendered.querySelector('.ansi-log-message');
    expect(messageEl!.innerHTML).not.toContain('<script>');
    expect(messageEl!.textContent).toContain('<script>');
  });
});
