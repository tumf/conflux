/**
 * Responsive behaviour.
 *
 * jsdom has no layout engine, so page overflow is verified the only way a
 * repository-local suite honestly can: by proving the stylesheet contains no rule
 * that can produce it (no fixed widths wider than the 320 CSS pixel floor, no
 * unwrapped long content, no horizontally scrolling page) and that every viewport
 * this console claims to support keeps all of its content and controls in the
 * DOM and operable.
 */

import { describe, expect, it } from 'vitest';

import { INDEX_HTML, STYLE_CSS, flush, mountConsole } from './helpers/console.js';
import { sampleSnapshot } from './helpers/server.js';

/** Declarations of the form `property: value` anywhere in the stylesheet. */
function declarations(property) {
  const pattern = new RegExp(`(^|[;{\\s])${property}\\s*:\\s*([^;}]+)`, 'g');
  return Array.from(STYLE_CSS.matchAll(pattern)).map((match) => match[2].trim());
}

const REM_PX = 16;

/** Convert a CSS length to pixels when it is statically knowable. */
function toPixels(value) {
  const px = /^(-?\d*\.?\d+)px$/.exec(value);
  if (px) return Number(px[1]);
  const rem = /^(-?\d*\.?\d+)rem$/.exec(value);
  if (rem) return Number(rem[1]) * REM_PX;
  return null;
}

describe('no page-level horizontal overflow is possible', () => {
  it('declares no fixed width wider than the 320px floor', () => {
    for (const value of [...declarations('width'), ...declarations('min-width')]) {
      const pixels = toPixels(value);
      if (pixels === null) continue;
      expect(pixels, `width ${value} exceeds the 320px floor`).toBeLessThanOrEqual(320);
    }
  });

  it('lets flexible children shrink instead of forcing a minimum content width', () => {
    // `min-width: 0` on flex/grid children is what stops a long unbroken token
    // from establishing a floor wider than the viewport.
    expect(STYLE_CSS).toMatch(/min-width:\s*0/);
    expect(STYLE_CSS).toMatch(/minmax\(0,\s*1fr\)/);
  });

  it('breaks long unbroken values rather than letting them extend the page', () => {
    expect(STYLE_CSS).toMatch(/overflow-wrap:\s*anywhere/);
    // `nowrap` is allowed only where the element is visually hidden.
    const nowrap = Array.from(STYLE_CSS.matchAll(/([^{}]*)\{[^}]*white-space:\s*nowrap[^}]*\}/g));
    for (const [, selector] of nowrap) {
      expect(selector.trim()).toBe('.visually-hidden');
    }
  });

  it('never scrolls the page sideways and bounds local scrolling instead', () => {
    expect(STYLE_CSS).not.toMatch(/overflow-x:\s*(scroll|auto)/);
    expect(STYLE_CSS).toMatch(/\.log-list\s*\{[^}]*overflow-y:\s*auto/);
  });

  it('sizes the dialog from the viewport rather than a fixed width', () => {
    expect(STYLE_CSS).toMatch(/width:\s*min\(32rem,\s*calc\(100vw/);
  });
});

describe('breakpoints', () => {
  it('is mobile-first: layout rules only add columns as width increases', () => {
    const widthQueries = Array.from(STYLE_CSS.matchAll(/@media \((min|max)-width: ([^)]+)\)/g));
    expect(widthQueries.length).toBeGreaterThan(0);
    for (const [, direction] of widthQueries) {
      expect(direction).toBe('min');
    }
    expect(STYLE_CSS).toMatch(/@media \(min-width: 40rem\)/);
    expect(STYLE_CSS).toMatch(/@media \(min-width: 64rem\)/);
  });

  it('adapts to short viewports such as mobile landscape', () => {
    expect(STYLE_CSS).toMatch(/@media \(max-height: 30rem\)/);
  });

  it('declares a zoom-friendly viewport with no scaling lock', () => {
    const viewport = /<meta name="viewport" content="([^"]+)"/.exec(INDEX_HTML)[1];
    expect(viewport).toContain('width=device-width');
    expect(viewport).toContain('initial-scale=1');
    expect(viewport).not.toContain('maximum-scale');
    expect(viewport).not.toContain('user-scalable=no');
  });

  it('never sets a root font size, so 200% zoom and user settings both work', () => {
    expect(STYLE_CSS).not.toMatch(/html\s*\{[^}]*font-size/);
    expect(STYLE_CSS).not.toMatch(/body\s*\{[^}]*font-size/);
  });
});

describe('touch targets', () => {
  it('gives every control the WCAG 2.2 minimum of 44 by 44 CSS pixels', () => {
    expect(STYLE_CSS).toMatch(/--touch-target:\s*44px/);
    const btn = /\.btn\s*\{([^}]*)\}/.exec(STYLE_CSS)[1];
    expect(btn).toMatch(/min-height:\s*var\(--touch-target\)/);
    expect(btn).toMatch(/min-width:\s*var\(--touch-target\)/);

    for (const selector of ['.tab', '.skip-link']) {
      const rule = new RegExp(`\\${selector}\\s*\\{([^}]*)\\}`).exec(STYLE_CSS)[1];
      expect(rule).toMatch(/min-height:\s*var\(--touch-target\)/);
    }
    const inputs = /input,\s*\nselect\s*\{([^}]*)\}/.exec(STYLE_CSS)[1];
    expect(inputs).toMatch(/min-height:\s*var\(--touch-target\)/);
  });
});

describe('content survives every supported viewport', () => {
  it('keeps long identifiers, paths, and errors in the DOM in full', async () => {
    const longId = 'change-with-an-extremely-long-identifier-'.repeat(4);
    const longMessage = 'a very long log line without any spaces:'.replace(/ /g, '-').repeat(20);
    const { app, doc, server } = mountConsole({
      snapshot: sampleSnapshot({
        changes: [
          {
            id: longId,
            display_status: 'error',
            progress_status: 'in_progress',
            completed_tasks: 1,
            total_tasks: 4,
            progress_percent: 25,
            dependencies: [longId],
          },
        ],
        totals: { total: 1, completed: 0, in_progress: 1, pending: 0 },
      }),
      worktrees: [
        {
          worktree_id: 'aaaabbbbccccddddeeeeffff00001111',
          repository_id: 'abcdef0123456789',
          path: '../a/very/deeply/nested/path/that/keeps/going/and/going/and/going/worktree',
          branch: 'cflx/an-extremely-long-branch-name-that-will-not-fit-on-a-phone',
          head: '0011223344556677889a',
          is_main: false,
          is_detached: false,
          dirty: false,
          has_commits_ahead: false,
          operations: { deletable: true, mergeable: false, merge_blocked_reason: 'nothing to merge' },
        },
      ],
      logs: [
        {
          timestamp: '12:00:00',
          created_at: 1700000000,
          message: longMessage,
          level: 'error',
          change_id: longId,
        },
      ],
    });
    await app.bootstrap();
    await flush();

    expect(doc.querySelector('.resource-title').textContent).toBe(longId);
    doc.querySelector('[aria-expanded]').click();
    expect(doc.querySelector('.resource-details').textContent).toContain(longId);
    expect(doc.getElementById('log-list').textContent).toContain(longMessage);

    doc.getElementById('tab-worktrees').click();
    expect(doc.querySelector('.resource-path').textContent).toContain('worktree');
    expect(doc.querySelector('.blocked-reason').textContent).toContain('nothing to merge');
    expect(server.requests.every((request) => request.path.startsWith('/api/v2/'))).toBe(true);
  });

  it('keeps every panel reachable from the tab list at any width', async () => {
    const { app, doc } = mountConsole();
    await app.bootstrap();
    await flush();

    for (const tabId of ['tab-changes', 'tab-worktrees', 'tab-logs']) {
      doc.getElementById(tabId).click();
      const tab = doc.getElementById(tabId);
      expect(tab.getAttribute('aria-selected')).toBe('true');
      expect(doc.getElementById(tab.getAttribute('aria-controls')).hidden).toBe(false);
    }
  });
});
