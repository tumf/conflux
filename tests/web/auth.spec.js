/**
 * Browser authentication: loopback no-token mode, the 401 token form, recovery
 * from a wrong token, reload continuity, disconnect clearing, and the rule that
 * the credential never leaves the Authorization header.
 */

import { describe, expect, it } from 'vitest';

import { APP_JS, flush, memoryStorage, mountConsole } from './helpers/console.js';
import { createApiClient, createTokenStore } from '../../web/app.js';
import { createFixtureServer } from './helpers/server.js';

describe('unauthenticated loopback deployment', () => {
  it('bootstraps and renders state without any credential', async () => {
    const { app, server, doc } = mountConsole({ requireAuth: false });

    const ok = await app.bootstrap();
    await flush();

    expect(ok).toBe(true);
    expect(app.authRequired).toBe(false);
    expect(doc.getElementById('auth-section').hidden).toBe(true);
    expect(doc.getElementById('status-mode').textContent).toBe('Running');
    expect(server.requests.every((request) => request.authorization === null)).toBe(true);
  });

  it('only ever calls versioned routes', async () => {
    const { app, server } = mountConsole({ requireAuth: false });
    await app.bootstrap();
    await flush();

    expect(server.requests.length).toBeGreaterThan(0);
    for (const request of server.requests) {
      expect(request.path.startsWith('/api/v2/')).toBe(true);
    }
  });
});

describe('authenticated deployment', () => {
  it('shows an accessible token form after an unauthorized bootstrap', async () => {
    const { app, doc, server } = mountConsole({ requireAuth: true, token: 'right-token' });

    const ok = await app.bootstrap();
    await flush();

    expect(ok).toBe(false);
    expect(app.authRequired).toBe(true);

    const section = doc.getElementById('auth-section');
    expect(section.hidden).toBe(false);
    expect(section.getAttribute('aria-labelledby')).toBe('auth-heading');

    const input = doc.getElementById('auth-token');
    expect(input.type).toBe('password');
    const label = doc.querySelector('label[for="auth-token"]');
    expect(label).not.toBeNull();
    expect(label.textContent.trim()).toBe('API token');
    expect(doc.getElementById('auth-error').textContent).toContain('unauthorized');

    // It must not keep hammering protected resources without a user action.
    const before = server.requests.length;
    await flush();
    expect(server.requests.length).toBe(before);
  });

  it('connects once the right token is submitted and sends it only as a header', async () => {
    const { app, doc, server, tokens } = mountConsole({ requireAuth: true, token: 'right-token' });
    await app.bootstrap();
    await flush();

    doc.getElementById('auth-token').value = 'right-token';
    doc.getElementById('auth-form').dispatchEvent(new doc.defaultView.Event('submit'));
    await flush();

    expect(app.authRequired).toBe(false);
    expect(doc.getElementById('status-mode').textContent).toBe('Running');
    expect(tokens.get()).toBe('right-token');

    const authenticated = server.requests.filter(
      (request) => request.path !== '/api/v2/health' && request.authorization !== null,
    );
    expect(authenticated.length).toBeGreaterThan(0);
    for (const request of authenticated) {
      expect(request.authorization).toBe('Bearer right-token');
    }
    // Never in a URL or query string, on any request.
    for (const request of server.requests) {
      expect(request.url).not.toContain('right-token');
      expect(request.search).not.toContain('right-token');
    }
    // Never rendered into the page either.
    expect(doc.body.textContent).not.toContain('right-token');
    expect(doc.getElementById('auth-token').value).toBe('');
  });

  it('recovers from a wrong token without retaining it', async () => {
    const { app, doc, tokens, storage } = mountConsole({ requireAuth: true, token: 'right-token' });
    await app.bootstrap();
    await flush();

    doc.getElementById('auth-token').value = 'wrong-token';
    doc.getElementById('auth-form').dispatchEvent(new doc.defaultView.Event('submit'));
    await flush();

    expect(app.authRequired).toBe(true);
    expect(tokens.get()).toBeNull();
    expect(storage.entries()).toEqual([]);
    expect(doc.getElementById('auth-section').hidden).toBe(false);

    doc.getElementById('auth-token').value = 'right-token';
    doc.getElementById('auth-form').dispatchEvent(new doc.defaultView.Event('submit'));
    await flush();

    expect(app.authRequired).toBe(false);
  });

  it('reuses a tab-scoped token across a reload', async () => {
    const { app } = mountConsole({
      requireAuth: true,
      token: 'right-token',
      seedToken: 'right-token',
    });

    const ok = await app.bootstrap();
    await flush();

    expect(ok).toBe(true);
    expect(app.authRequired).toBe(false);
  });

  it('clears in-memory and tab-scoped credentials on disconnect', async () => {
    const { app, doc, tokens, storage } = mountConsole({
      requireAuth: true,
      token: 'right-token',
      seedToken: 'right-token',
    });
    await app.bootstrap();
    await flush();
    expect(storage.entries().length).toBe(1);

    doc.getElementById('btn-disconnect').click();
    await flush();

    expect(tokens.get()).toBeNull();
    expect(storage.entries()).toEqual([]);
    expect(app.snapshot).toBeNull();
    expect(app.canMutate()).toBe(false);
    expect(doc.getElementById('auth-section').hidden).toBe(false);
    expect(doc.getElementById('connection-text').textContent).toBe('Not authenticated');
    expect(doc.getElementById('changes-groups').children.length).toBe(0);
    expect(doc.getElementById('log-list').children.length).toBe(0);
  });
});

describe('credential storage discipline', () => {
  it('never touches localStorage in the shipped client', () => {
    // Comments may name it - they explain why it is refused - but executable
    // code must never reach it.
    const code = APP_JS.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\/\/[^\n]*/g, '');
    expect(code).not.toMatch(/localStorage/);
    expect(code).toMatch(/sessionStorage/);
  });

  it('stores the token under a session-scoped key only', () => {
    const storage = memoryStorage();
    const tokens = createTokenStore(storage);
    tokens.set('a-token');
    expect(storage.entries()).toEqual([['cflx.console.token', 'a-token']]);
    tokens.clear();
    expect(storage.entries()).toEqual([]);
  });

  it('keeps working when storage throws', async () => {
    const hostile = {
      getItem() {
        throw new Error('storage disabled');
      },
      setItem() {
        throw new Error('storage disabled');
      },
      removeItem() {
        throw new Error('storage disabled');
      },
    };
    const tokens = createTokenStore(hostile);
    tokens.set('in-memory-only');
    expect(tokens.get()).toBe('in-memory-only');

    const server = createFixtureServer({ requireAuth: true, token: 'in-memory-only' });
    const api = createApiClient({ tokens, fetchImpl: server.fetch });
    await expect(api.state()).resolves.toMatchObject({ state_revision: 7 });
  });
});
