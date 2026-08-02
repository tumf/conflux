/**
 * Event observation: ordered SSE over `fetch()` streaming, replay-gap recovery,
 * process-incarnation change, malformed frames, disconnects, the polling
 * fallback, and the rule that mutations are refused whenever the displayed state
 * is not trusted.
 */

import { describe, expect, it } from 'vitest';

import { APP_JS, flush, mountConsole } from './helpers/console.js';
import { createSseParser } from '../../web/app.js';
import { STALE_AFTER_MS } from '../../web/app.js';

describe('SSE frame parsing', () => {
  it('assembles frames across chunk boundaries and reports keep-alives', () => {
    const parser = createSseParser();
    expect(parser.push('id: 1\nevent: state_ch')).toEqual([]);
    const frames = parser.push('anged\ndata: {"a":1}\n\n: keep-alive\n\n');
    expect(frames).toHaveLength(2);
    expect(frames[0]).toMatchObject({ comment: false, event: 'state_changed', data: '{"a":1}' });
    expect(frames[1].comment).toBe(true);
  });

  it('joins multi-line data payloads', () => {
    const parser = createSseParser();
    const [frame] = parser.push('data: {"a":\ndata: 1}\n\n');
    expect(frame.data).toBe('{"a":\n1}');
  });
});

describe('live observation', () => {
  it('opens the stream with the snapshot cursor and instance identity', async () => {
    const { app, server } = mountConsole();
    await app.bootstrap();
    await flush();

    const stream = server.requests.find((request) => request.path === '/api/v2/events');
    expect(stream).toBeDefined();
    expect(stream.search).toContain('after_sequence=12');
    expect(stream.search).toContain(`instance_id=${server.instanceId}`);
    expect(app.transport).toBe('stream');
    expect(app.freshness()).toBe('fresh');
  });

  it('re-reads the coherent snapshot when a state event arrives', async () => {
    const { app, server, doc } = mountConsole();
    await app.bootstrap();
    await flush();
    expect(app.stateRevision).toBe(7);

    server.advance((snapshot) => {
      snapshot.app_mode = 'stopping';
    });
    await flush();

    expect(app.stateRevision).toBe(8);
    expect(app.eventSequence).toBe(13);
    expect(doc.getElementById('status-mode').textContent).toBe('Stopping after current work');
  });

  it('appends log events without advancing the revision', async () => {
    const { app, server, doc } = mountConsole();
    await app.bootstrap();
    await flush();
    const before = app.stateRevision;

    server.emitLog({
      timestamp: '12:00:02',
      created_at: 1700000002,
      message: 'a streamed line',
      level: 'info',
      change_id: null,
    });
    await flush();

    expect(app.stateRevision).toBe(before);
    expect(doc.getElementById('log-list').textContent).toContain('a streamed line');
  });
});

describe('recovery', () => {
  it('refreshes the snapshot after a replay gap', async () => {
    const { app, server } = mountConsole();
    await app.bootstrap();
    await flush();
    server.revision = 42;
    server.sequence = 99;

    server.emitGap();
    await flush();

    expect(app.stateRevision).toBe(42);
    expect(app.eventSequence).toBe(100);
    expect(app.transport).toBe('stream');
  });

  it('refreshes when the process incarnation changes', async () => {
    const { app, server } = mountConsole();
    await app.bootstrap();
    await flush();
    const original = app.instanceId;

    server.instanceId = 'ffffffffffffffffffffffffffffffff';
    server.sequence += 1;
    server.emit({
      event_sequence: server.sequence,
      category: 'state',
      event_type: 'state_changed',
    });
    await flush();

    expect(app.instanceId).not.toBe(original);
    expect(app.instanceId).toBe('ffffffffffffffffffffffffffffffff');
  });

  it('refreshes when a frame cannot be parsed', async () => {
    const { app, server } = mountConsole();
    await app.bootstrap();
    await flush();
    server.revision = 21;

    server.emitRaw('event: state_changed\ndata: {not json\n\n');
    await flush();

    expect(app.stateRevision).toBe(21);
  });

  it('refreshes when sequences arrive out of order', async () => {
    const { app, server } = mountConsole();
    await app.bootstrap();
    await flush();
    server.revision = 31;

    server.sequence += 5;
    server.emit({
      event_sequence: server.sequence,
      category: 'state',
      event_type: 'state_changed',
    });
    await flush();

    expect(app.stateRevision).toBe(31);
  });

  it('reconnects with bounded backoff after the stream drops', async () => {
    const { app, server, clock } = mountConsole();
    await app.bootstrap();
    await flush();
    const streamRequests = () => server.requests.filter((r) => r.path === '/api/v2/events').length;
    expect(streamRequests()).toBe(1);

    server.dropStreams();
    await flush();
    expect(app.transport).toBe('reconnecting');
    expect(app.freshness()).toBe('reconnecting');

    clock.advance(500);
    await clock.runDue();
    await flush();

    expect(streamRequests()).toBe(2);
    expect(app.transport).toBe('stream');
  });

  it('falls back to no-store polling once reconnects are exhausted', async () => {
    const { app, server, clock } = mountConsole();
    await app.bootstrap();
    await flush();

    // Refuse every future stream so the backoff schedule runs out.
    const realFetch = server.fetch;
    server.fetch = async (url, init) => {
      if (String(url).includes('/api/v2/events')) throw new Error('stream unavailable');
      return realFetch(url, init);
    };
    app.api.openEventStream = async () => {
      throw new Error('stream unavailable');
    };

    server.dropStreams();
    await flush();
    for (let attempt = 0; attempt < 8; attempt += 1) {
      clock.advance(10_000);
      await clock.runDue();
      await flush();
    }

    expect(app.reconnectAttempts).toBeGreaterThan(6);
    server.revision = 55;
    await app.pollOnce();
    expect(app.transport).toBe('poll');
    expect(app.stateRevision).toBe(55);
  });
});

describe('freshness gating', () => {
  it('refuses mutations once the last confirmation is stale', async () => {
    const { app, server, clock, doc } = mountConsole();
    await app.bootstrap();
    await flush();
    expect(app.canMutate()).toBe(true);

    clock.advance(STALE_AFTER_MS + 1);
    app.render();

    expect(app.freshness()).toBe('stale');
    expect(app.canMutate()).toBe(false);
    expect(doc.getElementById('connection-text').textContent).toBe('Stale');
    expect(doc.getElementById('lifecycle-hint').hidden).toBe(false);
    for (const button of doc.querySelectorAll('#lifecycle-actions .btn')) {
      expect(button.disabled).toBe(true);
    }

    const before = server.commands.length;
    await app.submit({ intentId: 'stop', command: { type: 'stop' }, label: 'Stop' });
    expect(server.commands.length).toBe(before);
    expect(doc.getElementById('notification-list').textContent).toContain('Action refused');
  });

  it('reports disconnection and the last confirmed time', async () => {
    const { app, doc } = mountConsole();
    await app.bootstrap();
    await flush();

    app.transport = 'offline';
    app.render();

    expect(app.freshness()).toBe('disconnected');
    expect(doc.getElementById('connection-text').textContent).toBe('Disconnected');
    expect(doc.getElementById('connection-detail').textContent).toMatch(
      /^Last confirmed update: (?!never)/,
    );
    expect(app.canMutate()).toBe(false);
  });
});

describe('transport contract', () => {
  it('never uses EventSource or a browser WebSocket', () => {
    expect(APP_JS).not.toMatch(/new\s+EventSource/);
    expect(APP_JS).not.toMatch(/new\s+WebSocket/);
    expect(APP_JS).toMatch(/text\/event-stream/);
  });

  it('marks every request no-store so a snapshot is never served from cache', async () => {
    const { app, server } = mountConsole();
    await app.bootstrap();
    await flush();
    await app.pollOnce();

    expect(server.requests.length).toBeGreaterThan(4);
    for (const request of server.requests) {
      expect(request.cache).toBe('no-store');
    }
  });
});
