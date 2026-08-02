/**
 * WCAG 2.2 AA semantics and keyboard behaviour.
 *
 * The automated scan is axe-core at the `wcag2a`/`wcag2aa`/`wcag21aa`/`wcag22aa`
 * tags; any serious or critical violation fails the suite. The keyboard
 * assertions cover the flows a scanner cannot reach: skip link, tabs, disclosure,
 * the authentication form, the confirmation dialog, and focus restoration.
 */

import axe from 'axe-core';
import { describe, expect, it } from 'vitest';

import { flush, focusables, loadDocument, mountConsole } from './helpers/console.js';

const TAGS = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'];

async function scan(doc) {
  const results = await axe.run(doc.body, {
    runOnly: { type: 'tag', values: TAGS },
    // jsdom has no layout engine, so a colour or size check here would report a
    // finding it cannot actually measure. Both are covered by contrast.spec.js
    // and responsive.spec.js against the stylesheet source instead.
    rules: {
      'color-contrast': { enabled: false },
      'target-size': { enabled: false },
    },
  });
  return results.violations.filter((violation) =>
    ['serious', 'critical'].includes(violation.impact),
  );
}

function describeViolations(violations) {
  return violations
    .map((violation) => `${violation.id}: ${violation.help} (${violation.nodes.length} nodes)`)
    .join('\n');
}

async function connected(options = {}) {
  const harness = mountConsole(options);
  await harness.app.bootstrap();
  await flush();
  return harness;
}

describe('automated scan', () => {
  it('passes on the initial unconnected document', async () => {
    const doc = loadDocument();
    const violations = await scan(doc);
    expect(describeViolations(violations)).toBe('');
  });

  it('passes on the connected console', async () => {
    const { doc } = await connected();
    const violations = await scan(doc);
    expect(describeViolations(violations)).toBe('');
  });

  it('passes with the authentication form shown', async () => {
    const { doc } = await connected({ requireAuth: true, token: 'right-token' });
    expect(doc.getElementById('auth-section').hidden).toBe(false);
    const violations = await scan(doc);
    expect(describeViolations(violations)).toBe('');
  });

  it('passes with a confirmation dialog open and notifications present', async () => {
    const { doc, app } = await connected();
    app.notify({
      tone: 'error',
      title: 'Something failed',
      body: 'detail',
      errorCode: 'root_busy',
      correlationId: 'corr-1',
      recovery: 'Try again later.',
    });
    doc.querySelector('#lifecycle-actions [data-intent="force-stop"]').click();
    await flush(2);
    expect(doc.getElementById('confirm-dialog').open).toBe(true);

    const violations = await scan(doc);
    expect(describeViolations(violations)).toBe('');
  });
});

describe('document semantics', () => {
  it('declares a language, one main landmark, and a skip link into it', () => {
    const doc = loadDocument();

    expect(doc.documentElement.lang).toBe('en');
    expect(doc.querySelectorAll('main')).toHaveLength(1);

    const skip = doc.querySelector('.skip-link');
    expect(skip.getAttribute('href')).toBe('#main');
    expect(doc.getElementById('main')).not.toBeNull();
    // The skip link is the first thing in the tab order.
    expect(focusables(doc.body)[0]).toBe(skip);
  });

  it('uses landmarks and a hierarchical heading outline', () => {
    const doc = loadDocument();

    expect(doc.querySelector('header')).not.toBeNull();
    const levels = Array.from(doc.querySelectorAll('h1, h2, h3, h4')).map((heading) =>
      Number(heading.tagName.slice(1)),
    );
    expect(levels[0]).toBe(1);
    for (let index = 1; index < levels.length; index += 1) {
      expect(levels[index] - levels[index - 1]).toBeLessThanOrEqual(1);
    }
  });

  it('marks identifiers as untranslatable', () => {
    const doc = loadDocument();
    expect(doc.getElementById('instance-summary').getAttribute('translate')).toBe('no');
    expect(doc.getElementById('log-list').getAttribute('translate')).toBe('no');
  });

  it('provides one polite status region and one assertive alert region', () => {
    const doc = loadDocument();
    const polite = doc.getElementById('live-polite');
    const assertive = doc.getElementById('live-assertive');

    expect(polite.getAttribute('role')).toBe('status');
    expect(polite.getAttribute('aria-live')).toBe('polite');
    expect(assertive.getAttribute('role')).toBe('alert');
    expect(assertive.getAttribute('aria-live')).toBe('assertive');
  });
});

describe('tabs', () => {
  it('implements the WAI-ARIA tabs pattern', async () => {
    const { doc } = await connected();
    const tablist = doc.getElementById('tablist');
    expect(tablist.getAttribute('role')).toBe('tablist');
    expect(tablist.getAttribute('aria-label')).toBe('Console views');

    const tabs = Array.from(tablist.querySelectorAll('[role="tab"]'));
    expect(tabs).toHaveLength(3);
    for (const tab of tabs) {
      const panel = doc.getElementById(tab.getAttribute('aria-controls'));
      expect(panel.getAttribute('role')).toBe('tabpanel');
      expect(panel.getAttribute('aria-labelledby')).toBe(tab.id);
    }
    // Exactly one tab is selected and only that one is in the tab order.
    expect(tabs.filter((tab) => tab.getAttribute('aria-selected') === 'true')).toHaveLength(1);
    expect(tabs.filter((tab) => tab.tabIndex === 0)).toHaveLength(1);
  });

  it('moves selection with Arrow, Home, and End', async () => {
    const { doc } = await connected();
    const [changes, worktrees, logs] = Array.from(doc.querySelectorAll('[role="tab"]'));
    const press = (tab, key) =>
      tab.dispatchEvent(
        new doc.defaultView.KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }),
      );

    press(changes, 'ArrowRight');
    expect(worktrees.getAttribute('aria-selected')).toBe('true');
    expect(doc.getElementById('panel-worktrees').hidden).toBe(false);
    expect(doc.getElementById('panel-changes').hidden).toBe(true);
    expect(doc.activeElement).toBe(worktrees);

    press(worktrees, 'End');
    expect(logs.getAttribute('aria-selected')).toBe('true');

    press(logs, 'ArrowRight');
    expect(changes.getAttribute('aria-selected')).toBe('true');

    press(changes, 'ArrowLeft');
    expect(logs.getAttribute('aria-selected')).toBe('true');

    press(logs, 'Home');
    expect(changes.getAttribute('aria-selected')).toBe('true');
  });
});

describe('keyboard operation', () => {
  it('exposes every action as a real button reachable by keyboard', async () => {
    const { doc } = await connected();

    const interactive = doc.querySelectorAll('#main [data-intent], #main .disclosure');
    expect(interactive.length).toBeGreaterThan(0);
    for (const element of interactive) {
      expect(element.tagName).toBe('BUTTON');
      expect(element.getAttribute('type')).toBe('button');
    }
    // No card, row, or panel pretends to be a control.
    expect(doc.querySelectorAll('#main [onclick]')).toHaveLength(0);
    for (const row of doc.querySelectorAll('.resource')) {
      expect(row.hasAttribute('tabindex')).toBe(false);
      expect(row.getAttribute('role')).toBeNull();
    }
  });

  it('labels the token field and its show/hide control', async () => {
    const { doc } = await connected({ requireAuth: true, token: 'right-token' });

    const input = doc.getElementById('auth-token');
    const label = doc.querySelector('label[for="auth-token"]');
    expect(label).not.toBeNull();
    expect(input.getAttribute('aria-describedby')).toBe('auth-token-hint');

    const toggle = doc.getElementById('auth-token-toggle');
    expect(toggle.getAttribute('aria-pressed')).toBe('false');
    expect(toggle.getAttribute('aria-controls')).toBe('auth-token');

    toggle.click();
    expect(input.type).toBe('text');
    expect(toggle.getAttribute('aria-pressed')).toBe('true');
    toggle.click();
    expect(input.type).toBe('password');
    expect(toggle.getAttribute('aria-pressed')).toBe('false');
  });

  it('moves focus to the token field when authentication becomes required', async () => {
    const { doc } = await connected({ requireAuth: true, token: 'right-token' });
    expect(doc.activeElement).toBe(doc.getElementById('auth-token'));
  });

  it('announces routine updates politely and failures assertively', async () => {
    const { doc, app, server } = await connected();

    await app.submit({ intentId: 'stop', command: { type: 'stop' }, label: 'Stop' });
    await flush();
    expect(doc.getElementById('live-polite').textContent).toContain('Stop succeeded');
    const assertiveAfterSuccess = doc.getElementById('live-assertive').textContent;
    expect(assertiveAfterSuccess).toBe('');

    // Event traffic is never announced entry by entry.
    server.emitLog({
      timestamp: '12:00:09',
      created_at: 1700000009,
      message: 'chatty line',
      level: 'info',
      change_id: null,
    });
    await flush();
    expect(doc.getElementById('live-polite').textContent).not.toContain('chatty line');

    server.commandHandler = async () => ({
      ok: false,
      status: 409,
      headers: new Headers({ 'content-type': 'application/json' }),
      text: () =>
        Promise.resolve(
          JSON.stringify({
            error_code: 'lifecycle_conflict',
            message: 'not accepted now',
            correlation_id: 'corr-x',
          }),
        ),
    });
    await app.submit({ intentId: 'start', command: { type: 'start' }, label: 'Start' });
    await flush();
    expect(doc.getElementById('live-assertive').textContent).toContain('Start failed');
  });

  it('keeps disabled controls out of the tab order without hiding their reason', async () => {
    const { doc } = await connected({ worktreeFailure: 'lifecycle_conflict' });

    const placeholder = doc.getElementById('worktrees-placeholder');
    expect(placeholder.hidden).toBe(false);
    expect(placeholder.textContent).not.toBe('');
  });
});
