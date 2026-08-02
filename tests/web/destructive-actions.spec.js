/**
 * Destructive-action protection: force stop, active-change stop-and-dequeue, and
 * worktree delete all require an accessible confirmation, cancellation sends
 * nothing, confirmation sends exactly one opaque-ID command, and focus returns
 * to the control that opened the dialog.
 */

import { describe, expect, it } from 'vitest';

import { flush, mountConsole } from './helpers/console.js';

async function connected(options = {}) {
  const harness = mountConsole(options);
  await harness.app.bootstrap();
  await flush();
  return harness;
}

const DESTRUCTIVE = [
  {
    name: 'force stop',
    selector: '#lifecycle-actions [data-intent="force-stop"]',
    expectedType: 'force_stop',
  },
  {
    name: 'stop and dequeue an active change',
    selector: '[data-change-id="add-base-capability"] [data-intent^="stop-"]',
    expectedType: 'stop_and_dequeue',
  },
  {
    name: 'delete a worktree',
    selector: '[data-worktree-id="0f1e2d3c4b5a69788796a5b4c3d2e1f0"] [data-intent^="delete-"]',
    expectedType: 'delete_worktree',
  },
];

describe.each(DESTRUCTIVE)('$name', ({ selector, expectedType }) => {
  it('opens a dialog naming the target and the consequence', async () => {
    const { doc } = await connected();
    const invoker = doc.querySelector(selector);
    expect(invoker).not.toBeNull();

    invoker.click();
    await flush(2);

    const dialog = doc.getElementById('confirm-dialog');
    expect(dialog.open).toBe(true);
    expect(dialog.getAttribute('aria-labelledby')).toBe('confirm-title');
    expect(dialog.getAttribute('aria-describedby')).toBe('confirm-body');
    expect(doc.getElementById('confirm-body').textContent.length).toBeGreaterThan(20);
    // Cancel is the least destructive default and is where focus lands.
    expect(doc.activeElement).toBe(doc.getElementById('confirm-cancel'));
  });

  it('sends no command when cancelled and restores focus to the invoker', async () => {
    const { doc, server } = await connected();
    const invoker = doc.querySelector(selector);

    invoker.click();
    await flush(2);
    doc.getElementById('confirm-cancel').click();
    await flush(2);

    expect(server.commands).toHaveLength(0);
    expect(server.effects).toHaveLength(0);
    expect(doc.getElementById('confirm-dialog').open).toBe(false);
    expect(doc.activeElement).toBe(invoker);
  });

  it('sends no command when Escape closes the dialog', async () => {
    const { doc, server } = await connected();
    const invoker = doc.querySelector(selector);

    invoker.click();
    await flush(2);
    const dialog = doc.getElementById('confirm-dialog');
    dialog.dispatchEvent(new doc.defaultView.Event('cancel', { cancelable: true }));
    await flush(2);

    expect(server.commands).toHaveLength(0);
    expect(dialog.open).toBe(false);
    expect(doc.activeElement).toBe(invoker);
  });

  it('sends exactly one typed command when confirmed', async () => {
    const { doc, server } = await connected();
    const invoker = doc.querySelector(selector);

    invoker.click();
    await flush(2);
    const accept = doc.getElementById('confirm-accept');
    accept.click();
    // A second activation while the outcome is pending must do nothing.
    accept.click();
    await flush();

    expect(server.commands).toHaveLength(1);
    expect(server.commands[0].type).toBe(expectedType);
    expect(server.effects).toHaveLength(1);
    // The row is rebuilt by the state refresh, so the node identity changes but
    // focus must land back on the same control rather than at the document top.
    expect(doc.activeElement.dataset.intent).toBe(invoker.dataset.intent);
  });
});

describe('worktree mutation identity', () => {
  it('addresses delete and merge only by opaque worktree_id', async () => {
    const { doc, server } = await connected();

    for (const intent of ['delete-', 'merge-']) {
      const button = doc.querySelector(
        `[data-worktree-id="0f1e2d3c4b5a69788796a5b4c3d2e1f0"] [data-intent^="${intent}"]`,
      );
      button.click();
      await flush(2);
      doc.getElementById('confirm-accept').click();
      await flush();
    }

    expect(server.commands).toHaveLength(2);
    for (const command of server.commands) {
      expect(command.target).toEqual({ worktree_id: '0f1e2d3c4b5a69788796a5b4c3d2e1f0' });
      expect(JSON.stringify(command)).not.toContain('cflx/add-base-capability');
      expect(JSON.stringify(command)).not.toContain('worktrees/add-base-capability');
    }
  });

  it('never offers a mutation for a worktree the server marks ineligible', async () => {
    const { doc, server } = await connected();
    const blocked = doc.querySelector('[data-worktree-id="ffeeddccbbaa99887766554433221100"]');

    for (const button of blocked.querySelectorAll('[data-intent]')) {
      expect(button.disabled).toBe(true);
      button.click();
    }
    await flush(2);

    expect(doc.getElementById('confirm-dialog').open).toBe(false);
    expect(server.commands).toHaveLength(0);
  });
});

describe('non-destructive actions', () => {
  it('do not require a confirmation dialog', async () => {
    const { doc, server } = await connected();

    doc.querySelector('[data-change-id="fix-broken-thing"] [data-intent^="retry-"]').click();
    await flush();

    expect(doc.getElementById('confirm-dialog').open).toBe(false);
    expect(server.commands).toHaveLength(1);
    expect(server.commands[0].type).toBe('retry_change');
  });
});

describe('dialog reuse', () => {
  it('refuses to open a second dialog while one is unresolved', async () => {
    const { doc, app } = await connected();

    doc.querySelector('#lifecycle-actions [data-intent="force-stop"]').click();
    await flush(2);

    const second = await app.confirmDestructive({ title: 'Other', body: 'Other body' });
    expect(second).toBe(false);
    expect(doc.getElementById('confirm-title').textContent).toBe('Force stop?');
  });
});
