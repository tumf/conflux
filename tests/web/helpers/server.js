/**
 * An in-process `/api/v2` fixture.
 *
 * It reproduces the parts of the real contract the console depends on -
 * bearer authentication, coherent snapshots, ordered SSE, optimistic revisions,
 * idempotency replay, and typed errors - so a spec can assert real client
 * behaviour instead of asserting against a hand-written mock per test.
 *
 * Every request is recorded, which is what lets the specs prove that no legacy
 * route is ever touched and that the token never leaves the Authorization
 * header.
 */

export const INSTANCE_ID = 'a1b2c3d4e5f60718293a4b5c6d7e8f90';

/** A snapshot covering every operator-priority bucket. */
export function sampleSnapshot(overrides = {}) {
  return {
    app_mode: 'running',
    is_resolving: false,
    changes: [
      {
        id: 'fix-broken-thing',
        display_status: 'error',
        progress_status: 'in_progress',
        completed_tasks: 2,
        total_tasks: 8,
        progress_percent: 25,
        dependencies: ['add-base-capability'],
        iteration_number: 3,
      },
      {
        id: 'add-base-capability',
        display_status: 'applying',
        progress_status: 'in_progress',
        completed_tasks: 4,
        total_tasks: 10,
        progress_percent: 40,
        dependencies: [],
      },
      {
        id: 'queued-change',
        display_status: 'queued',
        progress_status: 'pending',
        completed_tasks: 0,
        total_tasks: 5,
        progress_percent: 0,
        dependencies: [],
      },
      {
        id: 'done-change',
        display_status: 'merged',
        progress_status: 'complete',
        completed_tasks: 6,
        total_tasks: 6,
        progress_percent: 100,
        dependencies: [],
      },
    ],
    totals: { total: 4, completed: 1, in_progress: 2, pending: 1 },
    ...overrides,
  };
}

/** Two worktrees: one fully eligible, one blocked by the server. */
export function sampleWorktrees() {
  return [
    {
      worktree_id: '0f1e2d3c4b5a69788796a5b4c3d2e1f0',
      repository_id: 'abcdef0123456789',
      path: '../worktrees/add-base-capability',
      branch: 'cflx/add-base-capability',
      head: '9f8e7d6c5b4a39281706',
      is_main: false,
      is_detached: false,
      dirty: false,
      has_commits_ahead: true,
      operations: { deletable: true, mergeable: true },
    },
    {
      worktree_id: 'ffeeddccbbaa99887766554433221100',
      repository_id: 'abcdef0123456789',
      path: '../worktrees/fix-broken-thing',
      branch: 'cflx/fix-broken-thing',
      head: '1122334455667788990a',
      is_main: false,
      is_detached: false,
      dirty: true,
      has_commits_ahead: false,
      conflict: { files: ['src/lib.rs'], recovery: 'local_or_tui_required' },
      operations: {
        deletable: false,
        mergeable: false,
        delete_blocked_reason: 'worktree has uncommitted changes',
        merge_blocked_reason: 'merging would conflict; resolve locally or in the TUI',
      },
    },
  ];
}

/** A log ring including an ANSI-coloured entry and a hostile-looking entry. */
export function sampleLogs() {
  return [
    {
      timestamp: '12:00:00',
      created_at: 1700000000,
      message: 'orchestration started',
      level: 'info',
      change_id: null,
      operation: null,
      iteration: null,
      workspace_path: null,
    },
    {
      timestamp: '12:00:01',
      created_at: 1700000001,
      message: '\u001b[31mapply failed\u001b[0m for fix-broken-thing',
      level: 'error',
      change_id: 'fix-broken-thing',
      operation: 'apply',
      iteration: 3,
      workspace_path: null,
    },
  ];
}

/** A pushable byte stream the fixture hands to `fetch()` response streaming. */
function createPushStream() {
  const queue = [];
  let waiting = null;
  let closed = false;

  const settle = () => {
    if (!waiting) return;
    if (queue.length > 0) {
      const resolve = waiting;
      waiting = null;
      resolve({ done: false, value: queue.shift() });
    } else if (closed) {
      const resolve = waiting;
      waiting = null;
      resolve({ done: true, value: undefined });
    }
  };

  return {
    push(text) {
      queue.push(text);
      settle();
    },
    close() {
      closed = true;
      settle();
    },
    get closed() {
      return closed;
    },
    body: {
      getReader() {
        return {
          read() {
            if (queue.length > 0) return Promise.resolve({ done: false, value: queue.shift() });
            if (closed) return Promise.resolve({ done: true, value: undefined });
            return new Promise((resolve) => {
              waiting = resolve;
            });
          },
          releaseLock() {},
          cancel() {
            closed = true;
            settle();
          },
        };
      },
    },
  };
}

function jsonResponse(status, body, headers = {}) {
  const merged = new Headers({ 'content-type': 'application/json', ...headers });
  return {
    ok: status >= 200 && status < 300,
    status,
    headers: merged,
    text: () => Promise.resolve(body === undefined ? '' : JSON.stringify(body)),
  };
}

/**
 * Build the fixture server.
 *
 * @param {object} [options]
 * @param {boolean} [options.requireAuth]
 * @param {string} [options.token]
 * @param {object} [options.snapshot]
 * @param {Array} [options.worktrees]
 * @param {Array} [options.logs]
 * @param {string|null} [options.worktreeFailure] typed error code for worktree reads
 */
export function createFixtureServer(options = {}) {
  const server = {
    requireAuth: options.requireAuth === true,
    token: options.token ?? 'fixture-token',
    instanceId: options.instanceId ?? INSTANCE_ID,
    revision: options.revision ?? 7,
    sequence: options.sequence ?? 12,
    snapshot: options.snapshot ?? sampleSnapshot(),
    worktrees: options.worktrees ?? sampleWorktrees(),
    logs: options.logs ?? sampleLogs(),
    worktreeFailure: options.worktreeFailure ?? null,
    /** Every request the console made, in order. */
    requests: [],
    /** Every command envelope the console posted, in order. */
    commands: [],
    /** Side effects the fixture actually executed. */
    effects: [],
    /** Bind a hook to control or fail one command submission. */
    commandHandler: null,
    /** Open SSE streams. */
    streams: [],
    idempotency: new Map(),
  };

  let commandSeq = 0;

  const unauthorized = (correlationId) =>
    jsonResponse(
      401,
      {
        error_code: 'unauthorized',
        message: 'missing or invalid bearer credentials',
        correlation_id: correlationId,
      },
      { 'x-correlation-id': correlationId },
    );

  const authorized = (init) => {
    if (!server.requireAuth) return true;
    const header = init?.headers?.get?.('Authorization') ?? null;
    return header === `Bearer ${server.token}`;
  };

  /** Publish one envelope to every open stream. */
  server.emit = (envelope) => {
    const full = {
      instance_id: server.instanceId,
      state_revision: server.revision,
      timestamp: '2026-08-02T00:00:00Z',
      payload: {},
      ...envelope,
    };
    const frame = `id: ${full.event_sequence}\nevent: ${full.event_type}\ndata: ${JSON.stringify(full)}\n\n`;
    for (const stream of server.streams) stream.push(frame);
    return full;
  };

  /** Advance the projection and announce it, exactly as the server does. */
  server.advance = (mutate) => {
    if (mutate) mutate(server.snapshot);
    server.revision += 1;
    server.sequence += 1;
    return server.emit({
      event_sequence: server.sequence,
      category: 'state',
      event_type: 'state_changed',
    });
  };

  /** Emit a log event without advancing the revision. */
  server.emitLog = (entry) => {
    server.sequence += 1;
    return server.emit({
      event_sequence: server.sequence,
      category: 'log',
      event_type: 'log',
      change_id: entry.change_id ?? null,
      payload: entry,
    });
  };

  /** Emit a replay-gap envelope. */
  server.emitGap = () => {
    server.sequence += 1;
    return server.emit({
      event_sequence: server.sequence,
      category: 'gap',
      event_type: 'gap',
    });
  };

  /** Send a raw frame, for malformed-stream coverage. */
  server.emitRaw = (frame) => {
    for (const stream of server.streams) stream.push(frame);
  };

  /** Drop every open stream, as a server restart or a proxy timeout would. */
  server.dropStreams = () => {
    for (const stream of server.streams) stream.close();
    server.streams = [];
  };

  server.fetch = async (url, init = {}) => {
    const parsed = new URL(url, 'http://127.0.0.1:8080');
    const correlationId = `corr-${server.requests.length}`;
    server.requests.push({
      url: String(url),
      path: parsed.pathname,
      search: parsed.search,
      method: init.method ?? 'GET',
      cache: init.cache ?? null,
      authorization: init.headers?.get?.('Authorization') ?? null,
      body: init.body ?? null,
    });

    if (parsed.pathname === '/api/v2/health') {
      return jsonResponse(200, { status: 'ok', api_version: 'v2', version: 'v0.6.214 (test)' });
    }

    if (!authorized(init)) return unauthorized(correlationId);

    switch (parsed.pathname) {
      case '/api/v2/capabilities':
        return jsonResponse(200, {
          api_version: 'v2',
          instance_id: server.instanceId,
          commands: [
            'start',
            'stop',
            'cancel_stop',
            'force_stop',
            'set_execution_mark',
            'set_queue_intent',
            'retry_change',
            'retry_errors',
            'stop_and_dequeue',
            'resolve_merge',
            'create_worktree',
            'delete_worktree',
            'merge_worktree',
          ],
          transports: [
            {
              name: 'sse',
              path: '/api/v2/events',
              client: 'fetch-response-streaming',
              browser_native_supported: false,
            },
          ],
          error_codes: ['unauthorized', 'stale_revision'],
          limits: {
            max_events: 1000,
            max_logs: 1000,
            max_commands: 1000,
            max_idempotency_records: 1000,
            command_record_ttl_secs: 86400,
            max_correlation_id_len: 64,
          },
          authentication_required: server.requireAuth,
          worktrees: {
            operations: ['list', 'detail', 'create', 'delete', 'merge'],
            merge_conflict_recovery: 'local_or_tui_required',
            merge_conflict_preserves_state: true,
            delete_requires_teardown: true,
          },
        });

      case '/api/v2/state':
        return jsonResponse(200, {
          instance_id: server.instanceId,
          state_revision: server.revision,
          event_sequence: server.sequence,
          snapshot: server.snapshot,
        });

      case '/api/v2/logs':
        return jsonResponse(200, {
          instance_id: server.instanceId,
          state_revision: server.revision,
          event_sequence: server.sequence,
          logs: server.logs,
        });

      case '/api/v2/worktrees':
        if (server.worktreeFailure) {
          return jsonResponse(409, {
            error_code: server.worktreeFailure,
            message: 'this instance has no worktree runtime bound yet',
            correlation_id: correlationId,
          });
        }
        return jsonResponse(200, {
          instance_id: server.instanceId,
          state_revision: server.revision,
          repository_id: 'abcdef0123456789',
          worktrees: server.worktrees,
        });

      case '/api/v2/events': {
        const stream = createPushStream();
        server.streams.push(stream);
        if (init.signal) {
          init.signal.addEventListener('abort', () => stream.close(), { once: true });
        }
        return {
          ok: true,
          status: 200,
          headers: new Headers({ 'content-type': 'text/event-stream' }),
          body: stream.body,
          text: () => Promise.resolve(''),
        };
      }

      case '/api/v2/commands':
        return handleCommand(init, correlationId);

      default:
        return jsonResponse(404, {
          error_code: 'not_found',
          message: `no such resource: ${parsed.pathname}`,
          correlation_id: correlationId,
        });
    }
  };

  async function handleCommand(init, correlationId) {
    const envelope = JSON.parse(init.body);
    server.commands.push(envelope);

    if (server.commandHandler) {
      const override = await server.commandHandler(envelope, { correlationId, jsonResponse });
      if (override) return override;
    }

    if (
      typeof envelope.idempotency_key !== 'string' ||
      envelope.idempotency_key.length < 1 ||
      envelope.idempotency_key.length > 200
    ) {
      return jsonResponse(422, {
        error_code: 'validation_failed',
        message: 'idempotency_key must be 1-200 characters',
        correlation_id: correlationId,
      });
    }

    // Replay before revision validation, exactly like the real admission order:
    // a retry stays safe even after the revision has moved on.
    const replay = server.idempotency.get(envelope.idempotency_key);
    if (replay) {
      if (JSON.stringify(replay.identity) !== JSON.stringify(identityOf(envelope))) {
        return jsonResponse(409, {
          error_code: 'idempotency_mismatch',
          message: 'idempotency_key is already bound to a different command identity',
          correlation_id: correlationId,
          current_revision: server.revision,
        });
      }
      return jsonResponse(200, replay.record);
    }

    if (envelope.expected_revision !== server.revision) {
      return jsonResponse(409, {
        error_code: 'stale_revision',
        message: `expected_revision ${envelope.expected_revision} is stale`,
        correlation_id: correlationId,
        current_revision: server.revision,
      });
    }

    commandSeq += 1;
    server.effects.push({ type: envelope.type, envelope });
    const record = {
      command_id: `cmd-${commandSeq}`,
      instance_id: server.instanceId,
      type: envelope.type,
      state: 'succeeded',
      expected_revision: envelope.expected_revision,
      result_revision: server.revision + 1,
      correlation_id: correlationId,
      idempotency_key: envelope.idempotency_key,
      created_at: '2026-08-02T00:00:00Z',
      completed_at: '2026-08-02T00:00:01Z',
      detail: `${envelope.type} applied`,
    };
    server.idempotency.set(envelope.idempotency_key, {
      identity: identityOf(envelope),
      record,
    });
    server.revision += 1;
    return jsonResponse(200, record);
  }

  function identityOf(envelope) {
    const { idempotency_key: _key, correlation_id: _correlation, ...identity } = envelope;
    return identity;
  }

  return server;
}
