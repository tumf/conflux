/**
 * Information architecture: what the first viewport says, how changes are
 * grouped, how details are disclosed, how worktree eligibility is presented, and
 * how logs render hostile content.
 */

import { describe, expect, it } from 'vitest';

import { flush, mountConsole } from './helpers/console.js';
import { classifyChange, groupChanges, lifecycleActions } from '../../web/app.js';
import { sampleSnapshot } from './helpers/server.js';

async function connected(options = {}) {
  const harness = mountConsole(options);
  await harness.app.bootstrap();
  await flush();
  return harness;
}

describe('operator priority classification', () => {
  it('buckets every canonical display status', () => {
    expect(classifyChange({ display_status: 'error' })).toBe('attention');
    expect(classifyChange({ display_status: 'stalled' })).toBe('attention');
    expect(classifyChange({ display_status: 'blocked' })).toBe('attention');
    expect(classifyChange({ display_status: 'merge wait' })).toBe('attention');
    expect(classifyChange({ display_status: 'rejected' })).toBe('attention');
    expect(classifyChange({ display_status: 'applying' })).toBe('active');
    expect(classifyChange({ display_status: 'archiving' })).toBe('active');
    expect(classifyChange({ display_status: 'queued' })).toBe('waiting');
    expect(classifyChange({ display_status: 'not queued' })).toBe('waiting');
    expect(classifyChange({ display_status: 'stopped' })).toBe('waiting');
    expect(classifyChange({ display_status: 'merged' })).toBe('completed');
    expect(classifyChange({ display_status: 'archived' })).toBe('completed');
    expect(classifyChange({})).toBe('waiting');
  });

  it('preserves input order inside a bucket', () => {
    const grouped = groupChanges([
      { id: 'b', display_status: 'error' },
      { id: 'a', display_status: 'error' },
    ]);
    expect(grouped.attention.map((change) => change.id)).toEqual(['b', 'a']);
  });
});

describe('next valid action', () => {
  it('offers stop while running, cancel while stopping, start otherwise', () => {
    expect(lifecycleActions({ app_mode: 'running' })[0]).toMatchObject({
      id: 'stop',
      primary: true,
    });
    expect(lifecycleActions({ app_mode: 'stopping' })[0]).toMatchObject({
      id: 'cancel-stop',
      primary: true,
    });
    for (const mode of ['select', 'stopped', 'error', '']) {
      expect(lifecycleActions({ app_mode: mode })[0]).toMatchObject({ id: 'start', primary: true });
    }
  });

  it('offers exactly one primary action in every mode', () => {
    for (const mode of ['select', 'running', 'stopping', 'stopped', 'error']) {
      const primary = lifecycleActions({ app_mode: mode }).filter((action) => action.primary);
      expect(primary).toHaveLength(1);
    }
  });
});

describe('first viewport', () => {
  it('states identity, freshness, mode, active work, attention, and the next action', async () => {
    const { doc } = await connected();

    expect(doc.getElementById('instance-summary').textContent).toContain('instance ');
    expect(doc.getElementById('instance-summary').textContent).toContain('revision 7');
    expect(doc.getElementById('connection-text').textContent).toBe('Live');
    expect(doc.getElementById('connection-detail').textContent).toMatch(/Last confirmed update: /);
    expect(doc.getElementById('status-mode').textContent).toBe('Running');
    expect(doc.getElementById('status-active').textContent).toBe('1 change running');
    expect(doc.getElementById('status-attention').textContent).toBe('1 change');
    expect(doc.getElementById('status-progress').textContent).toBe(
      '1 of 4 complete, 1 waiting',
    );

    const primary = doc.querySelector('#lifecycle-actions .btn-primary');
    expect(primary).not.toBeNull();
    expect(primary.textContent).toBe('Stop gracefully');
    expect(primary.disabled).toBe(false);
  });

  it('names the changes that need attention before any completion summary', async () => {
    const { doc } = await connected();

    const summary = doc.getElementById('attention-summary');
    expect(summary.hidden).toBe(false);
    expect(summary.textContent).toContain('fix-broken-thing');

    const main = doc.getElementById('main');
    const order = Array.from(main.querySelectorAll('#attention-summary, #status-progress'));
    expect(order[0].id).toBe('attention-summary');

    const groups = Array.from(doc.querySelectorAll('#changes-groups .change-group')).map(
      (group) => group.dataset.group,
    );
    expect(groups[0]).toBe('attention');
    expect(groups).toEqual(['attention', 'active', 'waiting', 'completed']);
  });

  it('hides the attention banner when nothing needs attention', async () => {
    const { doc } = await connected({
      snapshot: sampleSnapshot({
        changes: [
          {
            id: 'calm',
            display_status: 'queued',
            progress_status: 'pending',
            completed_tasks: 0,
            total_tasks: 3,
            progress_percent: 0,
            dependencies: [],
          },
        ],
        totals: { total: 1, completed: 0, in_progress: 0, pending: 1 },
      }),
    });

    expect(doc.getElementById('attention-summary').hidden).toBe(true);
    expect(doc.getElementById('status-attention').textContent).toBe('Nothing');
  });

  it('says so when the instance tracks no changes', async () => {
    const { doc } = await connected({
      snapshot: sampleSnapshot({
        app_mode: 'select',
        changes: [],
        totals: { total: 0, completed: 0, in_progress: 0, pending: 0 },
      }),
    });

    expect(doc.getElementById('changes-placeholder').hidden).toBe(false);
    expect(doc.getElementById('changes-placeholder').textContent).toContain('No changes');
    expect(doc.getElementById('changes-groups').children.length).toBe(0);
  });
});

describe('change rows', () => {
  it('exposes details through a labelled disclosure button, not a gesture', async () => {
    const { doc } = await connected();

    const row = doc.querySelector('[data-change-id="fix-broken-thing"]');
    const disclosure = row.querySelector('[aria-expanded]');
    expect(disclosure.tagName).toBe('BUTTON');
    expect(disclosure.textContent).toContain('fix-broken-thing');

    const details = doc.getElementById(disclosure.getAttribute('aria-controls'));
    expect(details.hidden).toBe(true);
    expect(disclosure.getAttribute('aria-expanded')).toBe('false');

    disclosure.click();
    expect(disclosure.getAttribute('aria-expanded')).toBe('true');
    expect(details.hidden).toBe(false);
    expect(details.textContent).toContain('add-base-capability');
    expect(details.textContent).toContain('Iteration');

    disclosure.click();
    expect(disclosure.getAttribute('aria-expanded')).toBe('false');
    expect(details.hidden).toBe(true);
  });

  it('keeps a disclosure open across a state refresh', async () => {
    const { doc, server, app } = await connected();
    doc.querySelector('[data-change-id="fix-broken-thing"] [aria-expanded]').click();

    server.advance();
    await flush();

    const disclosure = doc.querySelector('[data-change-id="fix-broken-thing"] [aria-expanded]');
    expect(disclosure.getAttribute('aria-expanded')).toBe('true');
    expect(app.expanded.has('fix-broken-thing')).toBe(true);
  });

  it('communicates status with a word and a shape, not colour alone', async () => {
    const { doc } = await connected();

    for (const badge of doc.querySelectorAll('#changes-groups .badge')) {
      const mark = badge.querySelector('.badge-mark');
      expect(mark.getAttribute('aria-hidden')).toBe('true');
      const words = badge.textContent.replace(mark.textContent, '').trim();
      expect(words.length).toBeGreaterThan(0);
    }
  });

  it('derives contextual actions from the current v2 status', async () => {
    const { doc } = await connected();

    const intents = (id) =>
      Array.from(doc.querySelectorAll(`[data-change-id="${id}"] [data-intent]`)).map(
        (button) => button.dataset.intent,
      );

    expect(intents('fix-broken-thing')).toContain('retry-fix-broken-thing');
    expect(intents('add-base-capability')).toContain('stop-add-base-capability');
    expect(intents('queued-change')).toContain('dequeue-queued-change');
    expect(intents('done-change')).toEqual([]);
  });

  it('describes long identifiers without truncating them out of the DOM', async () => {
    const longId = 'a-very-long-change-identifier-that-goes-on-and-on-'.repeat(3);
    const { doc } = await connected({
      snapshot: sampleSnapshot({
        changes: [
          {
            id: longId,
            display_status: 'queued',
            progress_status: 'pending',
            completed_tasks: 0,
            total_tasks: 1,
            progress_percent: 0,
            dependencies: [],
          },
        ],
        totals: { total: 1, completed: 0, in_progress: 0, pending: 1 },
      }),
    });

    // Matched by property rather than by an attribute selector: jsdom's selector
    // engine backtracks catastrophically on a very long attribute value.
    const row = Array.from(doc.querySelectorAll('[data-change-id]')).find(
      (element) => element.dataset.changeId === longId,
    );
    expect(row).toBeDefined();
    expect(row.querySelector('.resource-title').textContent).toBe(longId);
  });
});

describe('worktrees view', () => {
  it('renders server-provided eligibility and blocked reasons', async () => {
    const { doc } = await connected();

    const eligible = doc.querySelector('[data-worktree-id="0f1e2d3c4b5a69788796a5b4c3d2e1f0"]');
    expect(eligible.querySelector('[data-intent^="delete-"]').disabled).toBe(false);
    expect(eligible.querySelector('.blocked-reason')).toBeNull();

    const blocked = doc.querySelector('[data-worktree-id="ffeeddccbbaa99887766554433221100"]');
    expect(blocked.querySelector('[data-intent^="delete-"]').disabled).toBe(true);
    expect(blocked.querySelector('[data-intent^="merge-"]').disabled).toBe(true);
    const reasons = Array.from(blocked.querySelectorAll('.blocked-reason')).map(
      (element) => element.textContent,
    );
    expect(reasons.join(' ')).toContain('uncommitted changes');
    expect(reasons.join(' ')).toContain('resolve locally or in the TUI');
  });

  it('points conflict recovery at the local or TUI flow', async () => {
    const { doc } = await connected();
    const blocked = doc.querySelector('[data-worktree-id="ffeeddccbbaa99887766554433221100"]');
    expect(blocked.textContent).toContain('local_or_tui_required');
  });

  it('explains a refused worktree read instead of showing an empty list', async () => {
    const { doc } = await connected({ worktreeFailure: 'lifecycle_conflict' });

    const placeholder = doc.getElementById('worktrees-placeholder');
    expect(placeholder.hidden).toBe(false);
    expect(placeholder.textContent).toContain('lifecycle_conflict');
    expect(placeholder.textContent).toContain('no worktree runtime bound');
  });
});

describe('log panel', () => {
  it('renders supported ANSI colour as sanitized spans', async () => {
    const { doc } = await connected();

    const coloured = doc.querySelector('.log-message .ansi-fg-red');
    expect(coloured).not.toBeNull();
    expect(coloured.textContent).toBe('apply failed');
    expect(doc.getElementById('log-list').textContent).not.toContain('\u001b');
    expect(doc.getElementById('log-list').textContent).not.toContain('[31m');
  });

  it('keeps HTML in a log message as literal text', async () => {
    const { doc, server, app } = await connected();
    server.emitLog({
      timestamp: '12:00:03',
      created_at: 1700000003,
      message: '<img src=x onerror="globalThis.__pwned = true"><script>bad()</script>',
      level: 'warn',
      change_id: null,
    });
    await flush();

    const list = doc.getElementById('log-list');
    expect(list.querySelector('img')).toBeNull();
    expect(list.querySelector('script')).toBeNull();
    expect(globalThis.__pwned).toBeUndefined();
    expect(list.textContent).toContain('<img src=x');
    expect(app.logs.at(-1).level).toBe('warn');
  });

  it('filters by minimum level', async () => {
    const { doc } = await connected();
    const list = doc.getElementById('log-list');
    expect(list.children.length).toBe(2);

    const select = doc.getElementById('log-level');
    select.value = 'error';
    select.dispatchEvent(new doc.defaultView.Event('change'));

    expect(list.children.length).toBe(1);
    expect(list.textContent).toContain('apply failed');
    expect(doc.getElementById('log-count').textContent).toBe('1 of 2 entries');
  });
});

describe('degraded states', () => {
  it('explains itself when the instance cannot be reached at all', async () => {
    const { app, server, doc } = mountConsole();
    server.fetch = async () => {
      throw new TypeError('Failed to fetch');
    };
    app.api = (await import('../../web/app.js')).createApiClient({
      tokens: { get: () => null },
      fetchImpl: server.fetch,
    });

    const ok = await app.bootstrap();
    await flush();

    expect(ok).toBe(false);
    expect(app.freshness()).toBe('disconnected');
    expect(doc.getElementById('notification-list').textContent).toContain('Cannot reach');
    expect(doc.getElementById('lifecycle-hint').hidden).toBe(false);
  });
});
