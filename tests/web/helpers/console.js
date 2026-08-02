/**
 * Load the shipped console into a jsdom document and drive it.
 *
 * The specs run against `web/index.html`, `web/style.css`, and `web/app.js`
 * exactly as they are embedded in the binary - there is no test copy of the
 * markup to drift from production.
 */

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { createApiClient, createTokenStore, OperatorConsole } from '../../../web/app.js';
import { createFixtureServer } from './server.js';

const HERE = dirname(fileURLToPath(import.meta.url));
export const WEB_DIR = join(HERE, '..', '..', '..', 'web');

export const INDEX_HTML = readFileSync(join(WEB_DIR, 'index.html'), 'utf8');
export const STYLE_CSS = readFileSync(join(WEB_DIR, 'style.css'), 'utf8');
export const APP_JS = readFileSync(join(WEB_DIR, 'app.js'), 'utf8');

/**
 * Reparse the shipped document into the ambient jsdom document.
 *
 * @param {{withStyles?: boolean}} [options]
 * @returns {Document}
 */
export function loadDocument({ withStyles = false } = {}) {
  document.open();
  document.write(INDEX_HTML);
  document.close();
  if (withStyles) {
    const style = document.createElement('style');
    style.textContent = STYLE_CSS;
    document.head.appendChild(style);
  }
  return document;
}

/** A `sessionStorage`-shaped in-memory store. */
export function memoryStorage() {
  const map = new Map();
  return {
    getItem: (key) => (map.has(key) ? map.get(key) : null),
    setItem: (key, value) => map.set(key, String(value)),
    removeItem: (key) => map.delete(key),
    get size() {
      return map.size;
    },
    entries: () => Array.from(map.entries()),
  };
}

/** A controllable clock and timer set, so nothing in a spec waits on wall time. */
export function createClock(start = 1_700_000_000_000) {
  let current = start;
  const scheduled = new Map();
  let nextHandle = 1;
  return {
    now: () => current,
    advance(ms) {
      current += ms;
    },
    timers: {
      setTimeout(fn, ms) {
        const handle = nextHandle++;
        scheduled.set(handle, { fn, at: current + (ms ?? 0) });
        return handle;
      },
      clearTimeout(handle) {
        scheduled.delete(handle);
      },
    },
    /** Run every timer whose deadline has passed, oldest first. */
    async runDue() {
      const due = Array.from(scheduled.entries())
        .filter(([, entry]) => entry.at <= current)
        .sort((a, b) => a[1].at - b[1].at);
      for (const [handle, entry] of due) {
        scheduled.delete(handle);
        await entry.fn();
      }
    },
    pending: () => scheduled.size,
  };
}

/** Let queued microtasks and already-resolved promises settle. */
export async function flush(times = 6) {
  for (let index = 0; index < times; index += 1) {
    await Promise.resolve();
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
}

/**
 * Build a console wired to a fixture server.
 *
 * @param {object} [options] forwarded to `createFixtureServer`, plus `token`
 *   for a pre-seeded tab credential and `withStyles`.
 */
export function mountConsole(options = {}) {
  const doc = loadDocument({ withStyles: options.withStyles === true });
  const server = createFixtureServer(options);
  const storage = memoryStorage();
  const tokens = createTokenStore(storage);
  if (options.seedToken) tokens.set(options.seedToken);
  const clock = createClock();
  const api = createApiClient({ tokens, fetchImpl: server.fetch });
  const app = new OperatorConsole({
    document: doc,
    api,
    tokens,
    now: clock.now,
    timers: clock.timers,
    cryptoImpl: {
      // Deterministic keys make "the retry reused the same key" an exact
      // assertion rather than an inference.
      randomUUID: (() => {
        let counter = 0;
        return () => `idem-${++counter}`;
      })(),
    },
  });
  return { doc, server, tokens, storage, clock, api, app };
}

/** Every element that can receive keyboard focus, in document order. */
export function focusables(root) {
  return Array.from(
    root.querySelectorAll(
      'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])',
    ),
  ).filter((element) => !element.closest('[hidden]'));
}
