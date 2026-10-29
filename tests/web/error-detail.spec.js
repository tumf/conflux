/**
 * Change-local error details.
 *
 * An `error` row's Details disclosure carries the server's sanitised
 * `error_detail` verbatim under one `Error detail` term, or an explicit
 * `Error details unavailable` fallback. Non-error rows never show the term. The
 * disclosure choice survives coherent refreshes and replay-gap recovery, and
 * reading a diagnostic never authorises or submits a command.
 *
 * Every case runs the shipped console against the fixture `/api/v2` server.
 * Style coverage reads the shipped stylesheet's tokens and rules: jsdom has no
 * layout engine, so this is DOM and inherited-rule proof, not pixel layout,
 * native keyboard synthesis, or screen-reader observation.
 */

import { afterAll, describe, expect, it } from 'vitest';

import { STYLE_CSS, flush, mountConsole } from './helpers/console.js';
import { resolveToken, sliceAtRule, themes } from './helpers/css.js';
import { sampleActions, sampleSnapshot } from './helpers/server.js';

// The required case inventory, carried here so the gate never reads an active
// `openspec/changes/...` path. It mirrors the change's `ui-matrix.json`.
const CONTRACT_VERSION = 1;
const REQUIRED_ROWS = [
  'present',
  'fallback',
  'hostile',
  'bounded',
  'refresh-open',
  'refresh-closed',
  'clear',
  'become-error',
  'missing-after-present',
  'row-isolation',
  'reappear',
  'gap-recovery',
  'action-invariance',
  'inherited-style',
];
const VIEWPORTS = [
  { id: 'phone', width: 320, height: 640 },
  { id: 'tablet', width: 768, height: 1024 },
  { id: 'desktop', width: 1280, height: 800 },
  { id: 'short', width: 568, height: 320 },
  // 640 CSS px at 200% zoom lays out as a 320 CSS px viewport.
  { id: 'zoom', width: 320, height: 400 },
];

/** Case ID -> contract version, recorded only after a case's assertions pass. */
const executed = [];
function passed(id) {
  executed.push({ id, version: CONTRACT_VERSION });
}

const FALLBACK = 'Error details unavailable';
const REAL_DETAIL =
  "Change 'fixture-invalid-metadata' cannot start: invalid proposal metadata at " +
  'openspec/changes/fixture-invalid-metadata/proposal.md. Correct and commit that ' +
  'proposal, then explicitly Retry the change.';

function errorChange(id, errorDetail, extra = {}) {
  const change = {
    id,
    display_status: 'error',
    progress_status: 'in_progress',
    completed_tasks: 1,
    total_tasks: 4,
    progress_percent: 25,
    dependencies: ['add-base-capability'],
    iteration_number: 2,
    actions: sampleActions({ retry: true }),
    ...extra,
  };
  if (errorDetail !== undefined) change.error_detail = errorDetail;
  return change;
}

function snapshotWith(...changes) {
  const snapshot = sampleSnapshot();
  snapshot.changes = [...changes, ...snapshot.changes.filter((c) => c.id !== 'fix-broken-thing')];
  return snapshot;
}

async function connected(snapshot) {
  const harness = mountConsole({ withStyles: true, snapshot });
  await harness.app.bootstrap();
  await flush();
  return harness;
}

const row = (doc, id) => doc.querySelector(`li.resource[data-change-id="${id}"]`);
const disclosureOf = (doc, id) => row(doc, id).querySelector('button.disclosure');
const detailsOf = (doc, id) => row(doc, id).querySelector('.resource-details');
const terms = (doc, id) =>
  Array.from(detailsOf(doc, id).querySelectorAll('dl > div > dt')).map((dt) => dt.textContent);

/** Every `Error detail` wrapper inside one row's Details. */
function errorWrappers(doc, id) {
  return Array.from(detailsOf(doc, id).querySelectorAll('dl > div')).filter(
    (wrapper) => wrapper.querySelector('dt')?.textContent === 'Error detail',
  );
}

/** The single Error detail value, asserting its literal anatomy. */
function errorValue(doc, id) {
  const wrappers = errorWrappers(doc, id);
  expect(wrappers).toHaveLength(1);
  const [wrapper] = wrappers;
  expect(wrapper.children).toHaveLength(2);
  const [dt, dd] = wrapper.children;
  expect(dt.tagName).toBe('DT');
  expect(dd.tagName).toBe('DD');
  // Text only: no element children, no inline style, no role.
  expect(dd.children).toHaveLength(0);
  expect(dd.childNodes).toHaveLength(1);
  expect(dd.firstChild.nodeType).toBe(3);
  expect(dd.getAttribute('style')).toBeNull();
  expect(dd.getAttribute('role')).toBeNull();
  // It follows Dependencies, as the last term.
  const all = terms(doc, id);
  expect(all.at(-1)).toBe('Error detail');
  expect(all.at(-2)).toBe('Dependencies');
  return dd.textContent;
}

async function toggle(doc, id) {
  disclosureOf(doc, id).click();
  await flush(1);
}

function isOpen(doc, id) {
  const open = disclosureOf(doc, id).getAttribute('aria-expanded') === 'true';
  expect(detailsOf(doc, id).hidden).toBe(!open);
  return open;
}

describe('error detail content', () => {
  it('present: an expanded error row shows the supplied location and remedy verbatim', async () => {
    const { doc, server } = await connected(
      snapshotWith(errorChange('fixture-invalid-metadata', REAL_DETAIL)),
    );
    const id = 'fixture-invalid-metadata';
    expect(isOpen(doc, id)).toBe(false);
    const commands = server.commands.length;

    await toggle(doc, id);

    expect(isOpen(doc, id)).toBe(true);
    expect(terms(doc, id)).toEqual([
      'Status',
      'Task status',
      'Iteration',
      'Dependencies',
      'Error detail',
    ]);
    expect(errorValue(doc, id)).toBe(REAL_DETAIL);
    expect(server.commands.length).toBe(commands);
    passed('present');
  });

  it('fallback: absent, null, blank or nonstring detail shows the neutral fallback only', async () => {
    const inputs = [undefined, null, '', '   ', '\t\n ', 0, 42, true, false, {}, { a: 1 }, [], ['x']];
    const changes = inputs.map((value, index) => errorChange(`fallback-${index}`, value));
    const { doc } = await connected(snapshotWith(...changes));

    for (let index = 0; index < inputs.length; index += 1) {
      const id = `fallback-${index}`;
      await toggle(doc, id);
      expect(errorValue(doc, id), `input ${JSON.stringify(inputs[index])}`).toBe(FALLBACK);
      // No log line, activity or stringified object stands in for a cause.
      expect(detailsOf(doc, id).textContent).not.toContain('apply failed');
      expect(detailsOf(doc, id).textContent).not.toContain('[object');
    }
    passed('fallback');
  });

  it('hostile: HTML, ANSI-looking text and Unicode stay inert literal text', async () => {
    const hostile =
      '<img src=x onerror="window.__pwned=1"><script>window.__pwned=2</script>' +
      '<a href="javascript:alert(1)">fix</a> \u001b[31mred\u001b[0m [31m ' +
      '日本語 ✓ 🚀 é ‮evil‬  ';
    const { doc } = await connected(snapshotWith(errorChange('hostile-change', hostile)));

    await toggle(doc, 'hostile-change');

    // Verbatim, including trailing whitespace: trim decides presence only.
    expect(errorValue(doc, 'hostile-change')).toBe(hostile);
    const details = detailsOf(doc, 'hostile-change');
    expect(details.querySelector('img, script, a, span')).toBeNull();
    expect(window.__pwned).toBeUndefined();
    passed('hostile');
  });

  it('bounded: an 8,192-byte producer-bounded detail and a long unbroken path are not sliced', async () => {
    const marker = '…[truncated 120 bytes]';
    const encoder = new TextEncoder();
    let body = 'openspec/changes/fixture/proposal.md ';
    while (encoder.encode(body + 'é' + marker).length <= 8192) body += 'é';
    while (encoder.encode(body + marker).length < 8192) body += 'a';
    const bounded = body + marker;
    expect(encoder.encode(bounded).length).toBe(8192);
    const unbroken = `openspec/changes/${'x'.repeat(600)}/proposal.md`;

    const { doc } = await connected(
      snapshotWith(errorChange('bounded-change', bounded), errorChange('long-path', unbroken)),
    );
    await toggle(doc, 'bounded-change');
    await toggle(doc, 'long-path');

    expect(errorValue(doc, 'bounded-change')).toBe(bounded);
    expect(errorValue(doc, 'long-path')).toBe(unbroken);
    passed('bounded');
  });
});

describe('refresh transitions', () => {
  it('refresh-open: a new coherent detail replaces the old one and stays open', async () => {
    const { doc, server } = await connected(snapshotWith(errorChange('e1', 'first reason')));
    await toggle(doc, 'e1');

    server.advance((snapshot) => {
      snapshot.changes[0].error_detail = 'second reason';
    });
    await flush();

    expect(isOpen(doc, 'e1')).toBe(true);
    expect(errorValue(doc, 'e1')).toBe('second reason');
    expect(detailsOf(doc, 'e1').textContent).not.toContain('first reason');
    passed('refresh-open');
  });

  it('refresh-closed: a closed row stays closed across refresh until opened', async () => {
    const { doc, server } = await connected(snapshotWith(errorChange('e1', 'first reason')));

    server.advance((snapshot) => {
      snapshot.changes[0].error_detail = 'second reason';
    });
    await flush();

    expect(isOpen(doc, 'e1')).toBe(false);
    await toggle(doc, 'e1');
    expect(isOpen(doc, 'e1')).toBe(true);
    expect(errorValue(doc, 'e1')).toBe('second reason');
    passed('refresh-closed');
  });

  it('clear: becoming non-error omits a stale populated detail', async () => {
    const { doc, server } = await connected(snapshotWith(errorChange('e1', 'old reason')));
    await toggle(doc, 'e1');

    server.advance((snapshot) => {
      // The stale field remains in input; only the status moved.
      snapshot.changes[0].display_status = 'not queued';
    });
    await flush();

    expect(isOpen(doc, 'e1')).toBe(true);
    expect(errorWrappers(doc, 'e1')).toHaveLength(0);
    expect(terms(doc, 'e1')).toEqual(['Status', 'Task status', 'Iteration', 'Dependencies']);
    expect(detailsOf(doc, 'e1').textContent).not.toContain('old reason');
    passed('clear');
  });

  it('become-error: a non-error row that errors gains the detail while open', async () => {
    const { doc, server } = await connected(
      snapshotWith(errorChange('e1', 'stale reason', { display_status: 'not queued' })),
    );
    await toggle(doc, 'e1');
    expect(errorWrappers(doc, 'e1')).toHaveLength(0);

    server.advance((snapshot) => {
      snapshot.changes[0].display_status = 'error';
      snapshot.changes[0].error_detail = 'new reason';
    });
    await flush();

    expect(isOpen(doc, 'e1')).toBe(true);
    expect(errorValue(doc, 'e1')).toBe('new reason');
    passed('become-error');
  });

  it('missing-after-present: a removed detail becomes the fallback, never the old reason', async () => {
    const { doc, server } = await connected(snapshotWith(errorChange('e1', 'old reason')));
    await toggle(doc, 'e1');
    expect(errorValue(doc, 'e1')).toBe('old reason');

    server.advance((snapshot) => {
      delete snapshot.changes[0].error_detail;
    });
    await flush();

    expect(isOpen(doc, 'e1')).toBe(true);
    expect(errorValue(doc, 'e1')).toBe(FALLBACK);
    expect(detailsOf(doc, 'e1').textContent).not.toContain('old reason');
    passed('missing-after-present');
  });

  it('row-isolation: each row keeps its own disclosure and its own detail', async () => {
    const { doc, server } = await connected(
      snapshotWith(errorChange('e1', 'reason one'), errorChange('e2', 'reason two')),
    );
    await toggle(doc, 'e1');

    expect(isOpen(doc, 'e1')).toBe(true);
    expect(isOpen(doc, 'e2')).toBe(false);
    expect(errorValue(doc, 'e1')).toBe('reason one');
    expect(errorValue(doc, 'e2')).toBe('reason two');

    server.advance((snapshot) => {
      snapshot.changes[0].error_detail = 'reason one, revised';
    });
    await flush();

    expect(isOpen(doc, 'e1')).toBe(true);
    expect(isOpen(doc, 'e2')).toBe(false);
    expect(errorValue(doc, 'e1')).toBe('reason one, revised');
    expect(errorValue(doc, 'e2')).toBe('reason two');
    passed('row-isolation');
  });

  it('reappear: a row that disappears and returns keeps its same-console choice', async () => {
    const { doc, server, storage } = await connected(
      snapshotWith(errorChange('e1', 'reason'), errorChange('e2', 'reason')),
    );
    await toggle(doc, 'e1');
    let removed;

    server.advance((snapshot) => {
      removed = snapshot.changes.splice(0, 2);
    });
    await flush();
    expect(row(doc, 'e1')).toBeNull();
    expect(row(doc, 'e2')).toBeNull();

    server.advance((snapshot) => {
      snapshot.changes.unshift(...removed);
    });
    await flush();

    expect(isOpen(doc, 'e1')).toBe(true);
    expect(isOpen(doc, 'e2')).toBe(false);
    expect(errorValue(doc, 'e1')).toBe('reason');
    // Nothing durable: a new console starts collapsed and storage is untouched.
    expect(storage.entries().some(([key]) => /expand|disclos|detail/i.test(key))).toBe(false);
    const fresh = await connected(snapshotWith(errorChange('e1', 'reason')));
    expect(isOpen(fresh.doc, 'e1')).toBe(false);
    passed('reappear');
  });

  it('gap-recovery: a replay-gap resnapshot shows current detail and keeps open rows open', async () => {
    const { app, doc, server } = await connected(
      snapshotWith(errorChange('e1', 'before gap'), errorChange('e2', 'other')),
    );
    await toggle(doc, 'e1');

    // The change happened while events were lost; only the resnapshot carries it.
    server.snapshot.changes[0].error_detail = 'after gap';
    server.revision = 40;
    server.emitGap();
    await flush();

    expect(app.stateRevision).toBe(40);
    expect(isOpen(doc, 'e1')).toBe(true);
    expect(isOpen(doc, 'e2')).toBe(false);
    expect(errorValue(doc, 'e1')).toBe('after gap');
    passed('gap-recovery');
  });
});

describe('command authority', () => {
  it('action-invariance: Retry follows only server eligibility and mutation availability', async () => {
    const detail = 'identical diagnostic text. Explicitly Retry the change.';
    const { app, doc, server } = await connected(
      snapshotWith(
        errorChange('allowed', detail),
        errorChange('blocked', detail, {
          actions: sampleActions({ retry: false, blockedReason: 'apply_iteration_limit_active' }),
        }),
      ),
    );
    const retry = (id) => row(doc, id).querySelector(`[data-intent="retry-${id}"]`);
    const actionsOf = (id) =>
      Array.from(row(doc, id).querySelectorAll('.resource-actions button')).map((b) => [
        b.dataset.intent ?? b.dataset.disclosureFor,
        b.disabled,
      ]);
    const before = { allowed: actionsOf('allowed'), blocked: actionsOf('blocked') };
    const commandRequests = () =>
      server.requests.filter((request) => request.path === '/api/v2/commands').length;
    const mutable = app.canMutate();
    expect(mutable).toBe(true);

    expect(retry('allowed')).not.toBeNull();
    expect(retry('allowed').disabled).toBe(false);
    expect(retry('blocked')).toBeNull();

    for (const id of ['allowed', 'blocked']) {
      await toggle(doc, id);
      expect(errorValue(doc, id)).toBe(detail);
      await toggle(doc, id);
      await toggle(doc, id);
    }

    expect(server.commands).toHaveLength(0);
    expect(commandRequests()).toBe(0);
    expect(app.canMutate()).toBe(mutable);
    expect(actionsOf('allowed')).toEqual(before.allowed);
    expect(actionsOf('blocked')).toEqual(before.blocked);

    // Offline: the diagnostic stays readable, Retry stays present but disabled.
    app.transport = 'offline';
    app.render();
    expect(errorValue(doc, 'allowed')).toBe(detail);
    expect(retry('allowed').disabled).toBe(true);
    expect(retry('blocked')).toBeNull();
    await toggle(doc, 'allowed');
    expect(server.commands).toHaveLength(0);
    expect(commandRequests()).toBe(0);
    passed('action-invariance');
  });
});

describe('inherited style and semantics', () => {
  /** Declarations of the first rule whose selector list contains `selector`. */
  function rule(css, selector) {
    const pattern = /([^{}]+)\{([^{}]*)\}/g;
    for (const match of css.replace(/\/\*[\s\S]*?\*\//g, '').matchAll(pattern)) {
      const selectors = match[1].split(',').map((part) => part.trim());
      if (selectors.includes(selector)) {
        const declarations = new Map();
        for (const part of match[2].split(';')) {
          const index = part.indexOf(':');
          if (index === -1) continue;
          declarations.set(part.slice(0, index).trim(), part.slice(index + 1).trim());
        }
        return declarations;
      }
    }
    return null;
  }

  it('inherited-style: the new term reuses definition-list anatomy, shipped tokens and native disclosure', async () => {
    const { doc } = await connected(snapshotWith(errorChange('e1', REAL_DETAIL)));
    const button = disclosureOf(doc, 'e1');
    const details = detailsOf(doc, 'e1');

    // Native disclosure semantics.
    expect(button.tagName).toBe('BUTTON');
    expect(button.type).toBe('button');
    expect(button.className).toBe('btn btn-secondary disclosure');
    expect(button.getAttribute('tabindex')).toBeNull();
    expect(button.getAttribute('aria-controls')).toBe(details.id);
    expect(doc.querySelectorAll(`#${details.id}`)).toHaveLength(1);
    expect(button.getAttribute('aria-expanded')).toBe('false');
    expect(details.hidden).toBe(true);
    await toggle(doc, 'e1');
    expect(button.getAttribute('aria-expanded')).toBe('true');
    expect(details.hidden).toBe(false);
    expect(doc.activeElement === button || doc.activeElement === doc.body).toBe(true);

    // Anatomy: resource-details > dl > div > dt + dd, no new controls.
    expect(errorValue(doc, 'e1')).toBe(REAL_DETAIL);
    expect(details.querySelectorAll('button, a, [role], [aria-live], [style]')).toHaveLength(0);
    expect(details.children).toHaveLength(1);
    expect(details.firstElementChild.tagName).toBe('DL');

    // Rules the new value inherits.
    expect(rule(STYLE_CSS, '.resource-details').get('margin-top')).toBe('var(--space-3)');
    expect(rule(STYLE_CSS, 'dl').get('margin')).toBe('0');
    const dt = rule(STYLE_CSS, '.resource-details dt');
    expect(dt.get('color')).toBe('var(--color-text-muted)');
    expect(dt.get('font-size')).toBe('0.8125rem');
    const dd = rule(STYLE_CSS, '.resource-details dd');
    expect(dd.get('margin')).toBe('0 0 var(--space-2) 0');
    expect(dd.get('font-family')).toBe('var(--font-mono)');
    expect(dd.get('font-size')).toBe('0.8125rem');
    for (const property of ['white-space', 'max-height', 'overflow', 'text-overflow', 'width']) {
      expect(dd.has(property), `dd must not set ${property}`).toBe(false);
    }
    const body = rule(STYLE_CSS, 'body');
    expect(body.get('overflow-wrap')).toBe('anywhere');
    expect(body.get('line-height')).toBe('1.5');
    expect(body.get('color')).toBe('var(--color-text)');
    const btn = rule(STYLE_CSS, '.btn');
    expect(btn.get('min-height')).toBe('var(--touch-target)');
    expect(rule(STYLE_CSS, ':focus-visible').get('outline')).toBe(
      'var(--focus-width) solid var(--color-focus)',
    );

    // Shipped dark/light token values.
    const { dark, light } = themes(STYLE_CSS);
    const expected = {
      dark: { '--color-text-muted': '#b8c2d4', '--color-text': '#f5f7fa' },
      light: { '--color-text-muted': '#4a5468', '--color-text': '#10131a' },
    };
    for (const [name, map] of [
      ['dark', dark],
      ['light', light],
    ]) {
      for (const [token, value] of Object.entries(expected[name])) {
        expect(resolveToken(map, token), `${name} ${token}`).toBe(value);
      }
      expect(resolveToken(map, '--space-2')).toBe('0.5rem');
      expect(resolveToken(map, '--space-3')).toBe('0.75rem');
      expect(resolveToken(map, '--touch-target')).toBe('44px');
      expect(resolveToken(map, '--focus-width')).toBe('3px');
      expect(resolveToken(map, '--font-mono')).toMatch(/monospace$/);
    }

    // Every viewport class: no breakpoint block redefines the details value, so
    // the inherited rules above hold at each one. Not a rendered-layout claim.
    const breakpointBodies = [
      sliceAtRule(STYLE_CSS, '@media (min-width: 40rem)'),
      sliceAtRule(STYLE_CSS, '@media (min-width: 64rem)'),
      sliceAtRule(STYLE_CSS, '@media (max-height: 30rem)'),
    ];
    for (const viewport of VIEWPORTS) {
      const applicable = breakpointBodies.filter((css, index) => {
        if (css === null) return false;
        if (index === 0) return viewport.width >= 640;
        if (index === 1) return viewport.width >= 1024;
        return viewport.height <= 480;
      });
      for (const css of applicable) {
        expect(css, viewport.id).not.toMatch(/\bdd\b|\bdt\b|resource-details/);
      }
      expect(isOpen(doc, 'e1'), viewport.id).toBe(true);
    }
    passed('inherited-style');
  });
});

describe('matrix inventory', () => {
  it('every required row executed exactly once at the current contract version', () => {
    const ids = executed.map((entry) => entry.id);
    expect(executed.every((entry) => entry.version === CONTRACT_VERSION)).toBe(true);
    expect(new Set(ids).size).toBe(ids.length);
    expect([...ids].sort()).toEqual([...REQUIRED_ROWS].sort());
    expect(ids).not.toContain('NOT_EVALUATED');
  });
});

afterAll(() => {
  // A failed or missing case leaves its ID unrecorded, which the inventory test
  // above already rejects; this guards against the inventory test being dropped.
  expect(executed.length).toBe(REQUIRED_ROWS.length);
});
