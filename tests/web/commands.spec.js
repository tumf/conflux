/**
 * Command safety: one typed submission path, the latest confirmed revision, a
 * per-intent idempotency key, duplicate prevention while pending, same-key retry
 * only for an outcome-unknown transport failure, stale-revision resynchronization
 * without replay, and typed-error presentation.
 */

import { describe, expect, it } from 'vitest';

import { flush, mountConsole } from './helpers/console.js';
import { newIdempotencyKey } from '../../web/app.js';

async function connected(options = {}) {
  const harness = mountConsole(options);
  await harness.app.bootstrap();
  await flush();
  return harness;
}

describe('command envelope', () => {
  it('carries the latest confirmed revision and a fresh idempotency key', async () => {
    const { app, server } = await connected();

    await app.submit({ intentId: 'stop', command: { type: 'stop' }, label: 'Stop' });

    expect(server.commands).toHaveLength(1);
    expect(server.commands[0]).toMatchObject({
      type: 'stop',
      expected_revision: 7,
      idempotency_key: 'idem-1',
    });
    expect(server.requests.some((request) => request.path === '/api/v2/commands')).toBe(true);
  });

  it('uses a different key for each new intent', async () => {
    const { app, server } = await connected();
    await app.submit({ intentId: 'stop', command: { type: 'stop' } });
    await flush();
    await app.submit({ intentId: 'start', command: { type: 'start' } });

    const keys = server.commands.map((command) => command.idempotency_key);
    expect(new Set(keys).size).toBe(keys.length);
  });

  it('generates keys inside the 1-200 character contract', () => {
    const key = newIdempotencyKey();
    expect(key.length).toBeGreaterThanOrEqual(1);
    expect(key.length).toBeLessThanOrEqual(200);
  });
});

describe('duplicate prevention', () => {
  it('creates no second intent while the first is pending', async () => {
    let release;
    const gate = new Promise((resolve) => {
      release = resolve;
    });
    const { app, server, doc } = await connected();
    server.commandHandler = async () => {
      await gate;
      return null;
    };

    const first = app.submit({ intentId: 'stop', command: { type: 'stop' }, label: 'Stop' });
    await flush(2);

    expect(app.pending.has('stop')).toBe(true);
    const button = doc.querySelector('#lifecycle-actions [data-intent="stop"]');
    expect(button.disabled).toBe(true);
    expect(button.getAttribute('aria-busy')).toBe('true');

    const second = await app.submit({ intentId: 'stop', command: { type: 'stop' } });
    expect(second).toBeNull();

    release();
    await first;
    await flush();

    expect(server.commands).toHaveLength(1);
    expect(server.effects).toHaveLength(1);
  });
});

describe('outcome-unknown retry', () => {
  it('retries with the same envelope and produces exactly one effect', async () => {
    const { app, server } = await connected();
    let attempts = 0;
    const realFetch = server.fetch;
    server.fetch = async (url, init) => {
      if (String(url).endsWith('/api/v2/commands')) {
        attempts += 1;
        if (attempts === 1) {
          // The command lands, but the response never reaches the client.
          await realFetch(url, init);
          throw new TypeError('Failed to fetch');
        }
      }
      return realFetch(url, init);
    };
    app.api = (await import('../../web/app.js')).createApiClient({
      tokens: { get: () => null },
      fetchImpl: server.fetch,
    });

    const record = await app.submit({
      intentId: 'force-stop',
      command: { type: 'force_stop' },
      label: 'Force stop',
    });
    await flush();

    expect(attempts).toBe(2);
    expect(server.commands).toHaveLength(2);
    expect(server.commands[0].idempotency_key).toBe(server.commands[1].idempotency_key);
    // The server collapsed the retry onto the first admission.
    expect(server.effects).toHaveLength(1);
    expect(record.command_id).toBe('cmd-1');
  });

  it('does not retry a typed failure', async () => {
    const { app, server } = await connected();
    server.commandHandler = async (_envelope, { correlationId }) => ({
      ok: false,
      status: 409,
      headers: new Headers({ 'content-type': 'application/json' }),
      text: () =>
        Promise.resolve(
          JSON.stringify({
            error_code: 'lifecycle_conflict',
            message: 'the current lifecycle state does not accept this command',
            correlation_id: correlationId,
          }),
        ),
    });

    await app.submit({ intentId: 'start', command: { type: 'start' }, label: 'Start' });
    await flush();

    expect(server.commands).toHaveLength(1);
  });
});

describe('stale revision', () => {
  it('resynchronizes and asks for a new decision instead of replaying', async () => {
    const { app, server, doc } = await connected();

    // The projection moves on without the console noticing.
    server.revision = 11;
    const record = await app.submit({
      intentId: 'stop',
      command: { type: 'stop' },
      label: 'Stop',
    });
    await flush();

    expect(record).toBeNull();
    expect(server.effects).toHaveLength(0);
    expect(app.stateRevision).toBe(11);

    const notifications = doc.getElementById('notification-list').textContent;
    expect(notifications).toContain('Stop was not applied');
    expect(notifications).toContain('stale_revision');
    expect(notifications).toContain('Next step');
    // The user has to choose again; nothing was resubmitted.
    expect(server.commands).toHaveLength(1);
  });
});

describe('typed error presentation', () => {
  it('renders message, code, correlation ID, and a recovery step', async () => {
    const { app, doc, server } = await connected();
    server.commandHandler = async () => ({
      ok: false,
      status: 409,
      headers: new Headers({ 'content-type': 'application/json' }),
      text: () =>
        Promise.resolve(
          JSON.stringify({
            error_code: 'root_busy',
            message: 'the workspace root is busy with another operation',
            correlation_id: 'corr-abc123',
          }),
        ),
    });

    await app.submit({ intentId: 'start', command: { type: 'start' }, label: 'Start' });
    await flush();

    const notification = doc.querySelector('#notification-list .notification-error');
    expect(notification).not.toBeNull();
    expect(notification.textContent).toContain('the workspace root is busy');
    expect(notification.textContent).toContain('code root_busy');
    expect(notification.textContent).toContain('correlation corr-abc123');
    expect(notification.textContent).toContain('Another repository operation is running');
    expect(doc.getElementById('live-assertive').textContent).toContain('failed');
  });

  it('keeps a failure until it is dismissed', async () => {
    const { app, doc, server } = await connected();
    server.commandHandler = async () => ({
      ok: false,
      status: 503,
      headers: new Headers({ 'content-type': 'application/json' }),
      text: () =>
        Promise.resolve(
          JSON.stringify({
            error_code: 'registry_capacity',
            message: 'no command slot could be reserved',
            correlation_id: 'corr-cap',
          }),
        ),
    });

    await app.submit({ intentId: 'start', command: { type: 'start' }, label: 'Start' });
    await flush();
    expect(doc.querySelectorAll('#notification-list .notification-error')).toHaveLength(1);

    // Unrelated re-renders must not drop it.
    app.render();
    expect(doc.querySelectorAll('#notification-list .notification-error')).toHaveLength(1);

    doc.querySelector('#notification-list .notification-error button').click();
    expect(doc.querySelectorAll('#notification-list .notification-error')).toHaveLength(0);
  });

  it('settles the pending state from the command record', async () => {
    const { app, doc } = await connected();
    await app.submit({ intentId: 'stop', command: { type: 'stop' }, label: 'Stop' });
    await flush();

    expect(app.pending.size).toBe(0);
    expect(doc.getElementById('notification-list').textContent).toContain('Stop succeeded');
    expect(doc.getElementById('live-polite').textContent).toContain('Stop succeeded');
  });
});
