/**
 * Attention-summary visibility under the shipped stylesheet.
 *
 * `renderCurrentStatus` sets `attention-summary.hidden` from the current
 * coherent snapshot, but the author rule `.attention { display: flex }` beats
 * the user-agent `[hidden] { display: none }` rule. A boolean `hidden`
 * assertion therefore cannot tell a disappearing banner from one that keeps
 * naming an error the operator already recovered from. These specs load
 * `web/index.html`, `web/style.css`, and `web/app.js` exactly as embedded and
 * assert the computed display of the summary across the design inventory:
 *
 * - A: initial coherent snapshot with zero attention rows
 * - B: one current attention row
 * - C: B -> zero attention rows after recovery/archive, same console
 * - D: B -> the same row becomes non-attention
 * - E: C/D -> a new attention row appears
 *
 * jsdom resolves the cascade for `display` but has no layout engine, so these
 * are DOM computed-style observations, not pixel or real-browser proof.
 */

import { describe, expect, it } from 'vitest';

import { STYLE_CSS, flush, mountConsole } from './helpers/console.js';
import { sampleActions, sampleSnapshot } from './helpers/server.js';

/** A change row in the requested display status. */
function change(id, displayStatus, overrides = {}) {
  const attention = ['error', 'stalled', 'blocked', 'merge wait', 'rejected'].includes(
    displayStatus,
  );
  return {
    id,
    display_status: displayStatus,
    progress_status: attention ? 'in_progress' : 'pending',
    completed_tasks: 1,
    total_tasks: 4,
    progress_percent: 25,
    dependencies: [],
    actions: sampleActions({ retry: attention }),
    ...overrides,
  };
}

function snapshotOf(...changes) {
  return sampleSnapshot({
    changes,
    totals: { total: changes.length, completed: 0, in_progress: changes.length, pending: 0 },
  });
}

/** Mount the production console with styles enabled and reach a coherent snapshot. */
async function connected(snapshot) {
  const harness = mountConsole({ withStyles: true, snapshot });
  await harness.app.bootstrap();
  await flush();
  return harness;
}

const summaryOf = (doc) => doc.getElementById('attention-summary');
const displayOf = (doc) => doc.defaultView.getComputedStyle(summaryOf(doc)).display;

/** Replace the published changes and announce the new revision. */
async function publish(server, ...changes) {
  server.advance((snapshot) => {
    snapshot.changes = changes;
    snapshot.totals = {
      total: changes.length,
      completed: 0,
      in_progress: changes.length,
      pending: 0,
    };
  });
  await flush();
}

function expectHidden(doc) {
  expect(summaryOf(doc).hidden).toBe(true);
  // The hidden state must remove the rendered box, not merely flag it.
  expect(displayOf(doc)).toBe('none');
  expect(doc.getElementById('status-attention').textContent).toBe('Nothing');
}

function expectVisible(doc, ...names) {
  const summary = summaryOf(doc);
  expect(summary.hidden).toBe(false);
  expect(displayOf(doc)).toBe('flex');
  for (const name of names) expect(summary.textContent).toContain(name);
}

describe('stylesheet contract', () => {
  it('keeps the baseline flex anatomy of a visible attention summary', () => {
    const block = /\.attention\s*\{([^}]*)\}/.exec(STYLE_CSS);
    expect(block).not.toBeNull();
    const body = block[1];
    for (const declaration of [
      /display:\s*flex/,
      /align-items:\s*flex-start/,
      /gap:\s*var\(--space-2\)/,
      /padding:\s*var\(--space-3\)/,
      /margin-bottom:\s*var\(--space-4\)/,
      /color:\s*var\(--color-status-attention\)/,
      /border:\s*1px solid var\(--color-status-attention\)/,
      /border-left-width:\s*4px/,
      /border-radius:\s*var\(--radius\)/,
      /font-weight:\s*600/,
    ]) {
      expect(body).toMatch(declaration);
    }
  });

  it('overrides only the hidden attention state, never a global [hidden] reset', () => {
    expect(STYLE_CSS).toMatch(/\.attention\[hidden\]\s*\{\s*display:\s*none;?\s*\}/);
    // A bare `[hidden]` rule would restyle unreviewed surfaces; the override
    // must stay scoped to the attention summary.
    expect(STYLE_CSS).not.toMatch(/(^|[\s,}])\[hidden\]/);
  });
});

describe('attention summary visibility', () => {
  it('A: is not rendered on an initial snapshot with nothing needing attention', async () => {
    const { doc, server } = await connected(snapshotOf(change('calm', 'queued')));

    expectHidden(doc);
    expect(server.commands).toHaveLength(0);
  });

  it('B: renders the current attention row with the shipped flex summary', async () => {
    const { doc, server } = await connected(snapshotOf(change('fix-broken-thing', 'error')));

    expectVisible(doc, 'fix-broken-thing');
    expect(summaryOf(doc).textContent).toContain('1 change need attention');
    expect(doc.getElementById('status-attention').textContent).toBe('1 change');
    expect(server.commands).toHaveLength(0);
  });

  it('C: disappears after the attention row is recovered and archived on the same console', async () => {
    const { doc, server } = await connected(snapshotOf(change('fix-broken-thing', 'error')));
    expectVisible(doc, 'fix-broken-thing');

    await publish(server, change('fix-broken-thing', 'archived', { progress_status: 'complete' }));

    expectHidden(doc);
    // The renderer may keep the old text in the DOM; the stylesheet alone must
    // guarantee that nothing stale is rendered.
    expect(server.commands).toHaveLength(0);
  });

  it('C: disappears after the attention row leaves the snapshot entirely', async () => {
    const { doc, server } = await connected(snapshotOf(change('fix-broken-thing', 'error')));
    expectVisible(doc, 'fix-broken-thing');

    await publish(server);

    expectHidden(doc);
    expect(server.commands).toHaveLength(0);
  });

  it('D: disappears when the same row becomes non-attention work', async () => {
    const { doc, server } = await connected(snapshotOf(change('fix-broken-thing', 'error')));
    expectVisible(doc, 'fix-broken-thing');

    await publish(server, change('fix-broken-thing', 'applying'));

    expectHidden(doc);
    expect(doc.getElementById('status-active').textContent).toBe('1 change running');
    expect(server.commands).toHaveLength(0);
  });

  it('E: reappears with only the current names after a hidden transition', async () => {
    const { doc, server } = await connected(snapshotOf(change('fix-broken-thing', 'error')));
    expectVisible(doc, 'fix-broken-thing');

    // C: recovered, then D-shaped follow-up work, then a different failure.
    await publish(server, change('fix-broken-thing', 'archived', { progress_status: 'complete' }));
    expectHidden(doc);
    await publish(server, change('add-feature', 'applying'));
    expectHidden(doc);

    await publish(server, change('add-feature', 'stalled'));

    expectVisible(doc, 'add-feature');
    expect(summaryOf(doc).textContent).not.toContain('fix-broken-thing');
    expect(doc.getElementById('status-attention').textContent).toBe('1 change');
    expect(server.commands).toHaveLength(0);
  });

  it('submits no command across every visibility transition', async () => {
    const { doc, server } = await connected(snapshotOf(change('calm', 'queued')));
    expectHidden(doc);

    await publish(server, change('calm', 'error'));
    expectVisible(doc, 'calm');
    await publish(server, change('calm', 'merged', { progress_status: 'complete' }));
    expectHidden(doc);

    expect(server.commands).toHaveLength(0);
    expect(server.effects).toHaveLength(0);
    expect(server.requests.filter((request) => request.method !== 'GET')).toHaveLength(0);
  });
});
