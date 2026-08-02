/**
 * Conflux operator console.
 *
 * A dependency-free `/api/v2` client. Nothing here talks to the legacy
 * single-instance `/api/*` or `/ws` surface: every read, stream, error, and
 * mutation goes through the versioned contract, which is what gives the browser
 * bearer authentication, optimistic revisions, idempotency, typed errors, and a
 * resumable event cursor.
 *
 * The module has no import-time side effects other than the guarded autostart at
 * the very bottom, so tests can import the pieces and drive them against their
 * own document, fetch, storage, and clock.
 */

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** Every v2 path this console is allowed to touch. */
export const API = {
  health: '/api/v2/health',
  capabilities: '/api/v2/capabilities',
  instance: '/api/v2/instance',
  state: '/api/v2/state',
  logs: '/api/v2/logs',
  worktrees: '/api/v2/worktrees',
  commands: '/api/v2/commands',
  events: '/api/v2/events',
};

/** Session-scoped token key. Never `localStorage`: that outlives the tab. */
export const TOKEN_STORAGE_KEY = 'cflx.console.token';

/** A confirmation older than this makes the displayed state untrustworthy. */
export const STALE_AFTER_MS = 45000;

/** No-store snapshot poll interval used when the event stream is unavailable. */
export const POLL_INTERVAL_MS = 5000;

/** Reconnect backoff schedule, in milliseconds. The last entry repeats. */
export const RECONNECT_BACKOFF_MS = [500, 1000, 2000, 5000, 10000];

/** Reconnect attempts before the console falls back to polling. */
export const MAX_RECONNECT_ATTEMPTS = 6;

/** Display statuses that require an operator decision before work continues. */
export const ATTENTION_STATUSES = new Set([
  'error',
  'stalled',
  'blocked',
  'rejected',
  'merge wait',
]);

/** Display statuses that mean work is executing right now. */
export const ACTIVE_STATUSES = new Set([
  'applying',
  'accepting',
  'rejecting',
  'archiving',
  'resolving',
]);

/** Display statuses that are terminal successes. */
export const COMPLETED_STATUSES = new Set(['merged', 'archived', 'pushed', 'complete']);

/** The four operator-priority buckets, in render order. */
export const CHANGE_GROUPS = [
  { key: 'attention', title: 'Needs attention' },
  { key: 'active', title: 'Active' },
  { key: 'waiting', title: 'Waiting' },
  { key: 'completed', title: 'Completed' },
];

/** Log levels ordered by severity, for the minimum-level filter. */
const LOG_LEVEL_ORDER = { info: 0, success: 0, warn: 1, error: 2 };

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

/**
 * Bucket a change by what the operator has to do about it.
 *
 * @param {{display_status?: string}} change
 * @returns {'attention'|'active'|'waiting'|'completed'}
 */
export function classifyChange(change) {
  const status = String(change?.display_status ?? '').toLowerCase();
  if (ATTENTION_STATUSES.has(status)) return 'attention';
  if (ACTIVE_STATUSES.has(status)) return 'active';
  if (COMPLETED_STATUSES.has(status)) return 'completed';
  return 'waiting';
}

/**
 * Group changes into the operator-priority buckets, preserving input order.
 *
 * @param {Array<object>} changes
 * @returns {Record<string, Array<object>>}
 */
export function groupChanges(changes) {
  const grouped = { attention: [], active: [], waiting: [], completed: [] };
  for (const change of changes ?? []) grouped[classifyChange(change)].push(change);
  return grouped;
}

/**
 * The lifecycle actions that are valid for an application mode.
 *
 * Exactly one action is primary so the initial viewport can answer "what do I do
 * next" without asking the user to interpret a row of disabled buttons.
 *
 * @param {{app_mode?: string}} snapshot
 * @returns {Array<object>}
 */
export function lifecycleActions(snapshot) {
  const mode = String(snapshot?.app_mode ?? '').toLowerCase();
  const forceStop = {
    id: 'force-stop',
    label: 'Force stop',
    command: { type: 'force_stop' },
    primary: false,
    destructive: true,
    description: 'Stop every running change immediately without waiting for it to finish.',
  };

  if (mode === 'running') {
    return [
      {
        id: 'stop',
        label: 'Stop gracefully',
        command: { type: 'stop' },
        primary: true,
        description: 'Let running changes finish, then stop.',
      },
      forceStop,
    ];
  }
  if (mode === 'stopping') {
    return [
      {
        id: 'cancel-stop',
        label: 'Cancel stop',
        command: { type: 'cancel_stop' },
        primary: true,
        description: 'Keep processing instead of stopping.',
      },
      forceStop,
    ];
  }
  return [
    {
      id: 'start',
      label: 'Start processing',
      command: { type: 'start' },
      primary: true,
      description: 'Begin or resume processing queued changes.',
    },
  ];
}

/**
 * Human-readable summary of what the process is doing right now.
 *
 * @param {object|null} snapshot
 * @returns {string}
 */
export function describeMode(snapshot) {
  const mode = String(snapshot?.app_mode ?? '').toLowerCase();
  switch (mode) {
    case 'running':
      return 'Running';
    case 'stopping':
      return 'Stopping after current work';
    case 'stopped':
      return 'Stopped';
    case 'error':
      return 'Error';
    case 'select':
      return 'Idle';
    default:
      return mode ? mode : 'Unknown';
  }
}

/**
 * Generate an idempotency key for one intended side effect.
 *
 * The v2 contract accepts 1-200 characters. `crypto.randomUUID()` is used when
 * available; the fallback keeps the key unguessable enough to never collide with
 * another intent in the same tab.
 *
 * @param {Crypto} [cryptoImpl]
 * @returns {string}
 */
export function newIdempotencyKey(cryptoImpl) {
  const impl = cryptoImpl ?? (typeof globalThis !== 'undefined' ? globalThis.crypto : undefined);
  if (impl && typeof impl.randomUUID === 'function') return impl.randomUUID();
  if (impl && typeof impl.getRandomValues === 'function') {
    const bytes = impl.getRandomValues(new Uint8Array(16));
    return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
  }
  // Deliberately last: only reachable in an environment with no Web Crypto.
  return `k-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 12)}`;
}

/**
 * Incrementally parse an SSE byte stream into frames.
 *
 * Comment frames (`:` keep-alives) are reported too, because receiving one is
 * proof the connection is still alive.
 *
 * @returns {{push: (text: string) => Array<{comment: boolean, event: string, data: string}>}}
 */
export function createSseParser() {
  let buffer = '';
  return {
    push(text) {
      buffer += String(text).replace(/\r\n/g, '\n').replace(/\r/g, '\n');
      const frames = [];
      let index = buffer.indexOf('\n\n');
      while (index !== -1) {
        const raw = buffer.slice(0, index);
        buffer = buffer.slice(index + 2);
        frames.push(parseSseFrame(raw));
        index = buffer.indexOf('\n\n');
      }
      return frames;
    },
  };
}

function parseSseFrame(raw) {
  const data = [];
  let event = 'message';
  let comment = true;
  for (const line of raw.split('\n')) {
    if (line === '') continue;
    if (line.startsWith(':')) continue;
    comment = false;
    const colon = line.indexOf(':');
    const field = colon === -1 ? line : line.slice(0, colon);
    let value = colon === -1 ? '' : line.slice(colon + 1);
    if (value.startsWith(' ')) value = value.slice(1);
    if (field === 'data') data.push(value);
    else if (field === 'event') event = value;
  }
  return { comment, event, data: data.join('\n') };
}

const ANSI_COLORS = ['black', 'red', 'green', 'yellow', 'blue', 'magenta', 'cyan', 'white'];

/**
 * Matches an SGR sequence, any other CSI sequence, or a bare two-byte escape.
 *
 * Every alternative is anchored on ESC, so ordinary text containing `[`, `_`, or
 * a capital letter is never mistaken for a control sequence.
 */
// eslint-disable-next-line no-control-regex
const ANSI_PATTERN = /\x1b\[([0-9;]*)m|\x1b\[[0-?]*[ -\/]*[@-~]|\x1b[@-Z\\-_]/g;

/**
 * Split a string carrying ANSI SGR sequences into styled text runs.
 *
 * Only SGR (`ESC [ ... m`) is interpreted; every other escape sequence is
 * dropped. The result is plain data - the caller turns it into text nodes, so no
 * log content can ever reach an HTML parser.
 *
 * @param {string} text
 * @returns {Array<{text: string, classes: Array<string>}>}
 */
export function parseAnsi(text) {
  const source = String(text ?? '');
  const runs = [];
  let cursor = 0;
  let active = { fg: null, bg: null, bold: false, underline: false, dim: false };
  const emit = (chunk) => {
    if (!chunk) return;
    runs.push({ text: chunk, classes: ansiClasses(active) });
  };

  ANSI_PATTERN.lastIndex = 0;
  let match = ANSI_PATTERN.exec(source);
  while (match !== null) {
    emit(source.slice(cursor, match.index));
    cursor = match.index + match[0].length;
    if (match[1] !== undefined) active = applySgr(active, match[1]);
    match = ANSI_PATTERN.exec(source);
  }
  emit(source.slice(cursor));
  return runs;
}

function ansiClasses(style) {
  const classes = [];
  if (style.fg) classes.push(`ansi-fg-${style.fg}`);
  if (style.bg) classes.push(`ansi-bg-${style.bg}`);
  if (style.bold) classes.push('ansi-bold');
  if (style.dim) classes.push('ansi-dim');
  if (style.underline) classes.push('ansi-underline');
  return classes;
}

function applySgr(style, params) {
  const next = { ...style };
  const codes = params === '' ? [0] : params.split(';').map((value) => Number(value) || 0);
  for (const code of codes) {
    if (code === 0) {
      next.fg = null;
      next.bg = null;
      next.bold = false;
      next.dim = false;
      next.underline = false;
    } else if (code === 1) next.bold = true;
    else if (code === 2) next.dim = true;
    else if (code === 4) next.underline = true;
    else if (code === 22) {
      next.bold = false;
      next.dim = false;
    } else if (code === 24) next.underline = false;
    else if (code === 39) next.fg = null;
    else if (code === 49) next.bg = null;
    else if (code >= 30 && code <= 37) next.fg = ANSI_COLORS[code - 30];
    else if (code >= 90 && code <= 97) next.fg = `bright-${ANSI_COLORS[code - 90]}`;
    else if (code >= 40 && code <= 47) next.bg = ANSI_COLORS[code - 40];
    else if (code >= 100 && code <= 107) next.bg = `bright-${ANSI_COLORS[code - 100]}`;
  }
  return next;
}

/**
 * Render ANSI-bearing text into a document fragment of styled spans.
 *
 * Uses `createTextNode` for every character of log content, so markup in a log
 * message stays inspectable text and can never execute.
 *
 * @param {Document} doc
 * @param {string} text
 * @returns {DocumentFragment}
 */
export function renderAnsi(doc, text) {
  const fragment = doc.createDocumentFragment();
  for (const run of parseAnsi(text)) {
    if (run.classes.length === 0) {
      fragment.appendChild(doc.createTextNode(run.text));
      continue;
    }
    const span = doc.createElement('span');
    span.className = run.classes.join(' ');
    span.textContent = run.text;
    fragment.appendChild(span);
  }
  return fragment;
}

/**
 * Recovery guidance for a typed v2 error code.
 *
 * @param {string} code
 * @returns {string}
 */
export function recoveryFor(code) {
  switch (code) {
    case 'unauthorized':
      return 'Enter a valid API token to continue.';
    case 'forbidden':
      return 'This origin is not allowed to control the instance. Open the console from an allowed origin.';
    case 'stale_revision':
      return 'The state moved on. Review the refreshed state and decide again.';
    case 'lifecycle_conflict':
      return 'The instance is not in a state that accepts this action right now.';
    case 'target_ineligible':
      return 'The target cannot accept this action right now.';
    case 'root_busy':
      return 'Another repository operation is running. Try again once it finishes.';
    case 'idempotency_mismatch':
      return 'A different action already used this key. Refresh and retry from the current state.';
    case 'registry_capacity':
      return 'The instance cannot accept another command yet. Wait for running commands to settle.';
    case 'validation_failed':
      return 'The request was rejected before execution. Refresh and try again.';
    case 'worktree_exists':
      return 'A worktree already exists for this change.';
    case 'worktree_not_found':
      return 'This worktree no longer exists. Refresh the worktree list.';
    case 'worktree_dirty':
    case 'worktree_dirty_unknown':
      return 'Commit, stash, or clean the worktree locally before deleting it.';
    case 'merge_conflict':
      return 'The merge conflicted and its intermediate state was preserved. Resolve it locally or in the TUI.';
    case 'not_found':
      return 'The resource is not present in this instance. Refresh and try again.';
    case 'transport_error':
      return 'Check that the instance is still running, then refresh the console.';
    default:
      return 'Refresh the console and try again.';
  }
}

// ---------------------------------------------------------------------------
// Token store
// ---------------------------------------------------------------------------

/**
 * Tab-scoped token storage.
 *
 * `sessionStorage` only, so the token dies with the tab; `localStorage` is never
 * touched. A storage failure (private mode, disabled storage) degrades to
 * in-memory rather than breaking the console.
 *
 * @param {Storage|null} [storage]
 */
export function createTokenStore(storage) {
  const read = () => {
    try {
      return storage ? storage.getItem(TOKEN_STORAGE_KEY) : null;
    } catch {
      return null;
    }
  };
  const write = (value) => {
    try {
      if (!storage) return;
      if (value === null) storage.removeItem(TOKEN_STORAGE_KEY);
      else storage.setItem(TOKEN_STORAGE_KEY, value);
    } catch {
      /* storage unavailable; the in-memory copy is authoritative */
    }
  };

  let memory = read();
  return {
    get: () => memory,
    set(value) {
      memory = value || null;
      write(memory);
    },
    clear() {
      memory = null;
      write(null);
    },
  };
}

// ---------------------------------------------------------------------------
// API client
// ---------------------------------------------------------------------------

/** A typed v2 failure, or a transport failure whose outcome is unknown. */
export class ApiFailure extends Error {
  /**
   * @param {{errorCode: string, message: string, correlationId?: string|null, currentRevision?: number|null, status?: number, outcomeUnknown?: boolean}} init
   */
  constructor(init) {
    super(init.message);
    this.name = 'ApiFailure';
    this.errorCode = init.errorCode;
    this.correlationId = init.correlationId ?? null;
    this.currentRevision = init.currentRevision ?? null;
    this.status = init.status ?? 0;
    /** True when the request may or may not have taken effect on the server. */
    this.outcomeUnknown = init.outcomeUnknown === true;
  }
}

/**
 * Build the `/api/v2` client.
 *
 * The token is read through `tokens` on every call and is placed only in the
 * `Authorization` header - never in a URL, a query parameter, a correlation ID,
 * or anything the console renders.
 *
 * @param {{fetchImpl?: Function, tokens: object, baseUrl?: string}} options
 */
export function createApiClient({ fetchImpl, tokens, baseUrl = '' }) {
  const doFetch = fetchImpl ?? ((...args) => globalThis.fetch(...args));

  const headers = (extra) => {
    const result = new Headers(extra ?? {});
    const token = tokens.get();
    if (token) result.set('Authorization', `Bearer ${token}`);
    return result;
  };

  /**
   * Issue a request and normalize both typed and transport failures.
   *
   * @param {string} path
   * @param {{method?: string, body?: object, anonymous?: boolean, signal?: AbortSignal}} [options]
   */
  async function request(path, options = {}) {
    const method = options.method ?? 'GET';
    const init = {
      method,
      cache: 'no-store',
      headers: options.anonymous ? new Headers() : headers(),
      signal: options.signal,
    };
    if (options.body !== undefined) {
      init.headers.set('Content-Type', 'application/json');
      init.body = JSON.stringify(options.body);
    }

    let response;
    try {
      response = await doFetch(baseUrl + path, init);
    } catch (error) {
      // The request never produced a response, so for a mutation the server may
      // still have executed it. Callers retry these with the *same* key.
      throw new ApiFailure({
        errorCode: 'transport_error',
        message: error?.message
          ? `Network request failed: ${error.message}`
          : 'Network request failed',
        outcomeUnknown: method !== 'GET',
      });
    }

    const text = await response.text();
    let payload = null;
    if (text) {
      try {
        payload = JSON.parse(text);
      } catch {
        payload = null;
      }
    }

    if (!response.ok) {
      throw new ApiFailure({
        errorCode: payload?.error_code ?? `http_${response.status}`,
        message: payload?.message ?? `Request to ${path} failed with status ${response.status}`,
        correlationId: payload?.correlation_id ?? response.headers?.get?.('x-correlation-id') ?? null,
        currentRevision: payload?.current_revision ?? null,
        status: response.status,
      });
    }
    return payload;
  }

  return {
    request,
    health: () => request(API.health, { anonymous: true }),
    capabilities: () => request(API.capabilities),
    state: () => request(API.state),
    logs: () => request(API.logs),
    worktrees: () => request(API.worktrees),
    command: (envelope) => request(API.commands, { method: 'POST', body: envelope }),

    /**
     * Open the authenticated SSE stream with `fetch()` response streaming.
     *
     * `EventSource` cannot attach an Authorization header, so it is not an
     * option for authenticated v2 - the capabilities resource says as much.
     *
     * @param {{afterSequence?: number|null, instanceId?: string|null, signal?: AbortSignal}} [params]
     */
    async openEventStream({ afterSequence = null, instanceId = null, signal } = {}) {
      const query = new URLSearchParams();
      if (afterSequence !== null && afterSequence !== undefined) {
        query.set('after_sequence', String(afterSequence));
      }
      if (instanceId) query.set('instance_id', instanceId);
      const suffix = query.toString() ? `?${query}` : '';

      const response = await doFetch(baseUrl + API.events + suffix, {
        method: 'GET',
        cache: 'no-store',
        headers: headers({ Accept: 'text/event-stream' }),
        signal,
      });
      if (!response.ok || !response.body) {
        throw new ApiFailure({
          errorCode: response.status === 401 ? 'unauthorized' : `http_${response.status}`,
          message: `Event stream refused with status ${response.status}`,
          status: response.status,
        });
      }
      return { status: response.status, frames: readSseFrames(response.body, signal) };
    },
  };
}

async function* readSseFrames(body, signal) {
  const parser = createSseParser();
  const decoder = new TextDecoder();
  const reader = body.getReader();
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) return;
      const text = typeof value === 'string' ? value : decoder.decode(value, { stream: true });
      for (const frame of parser.push(text)) yield frame;
      if (signal && signal.aborted) return;
    }
  } finally {
    try {
      reader.releaseLock();
    } catch {
      /* the reader is already released */
    }
  }
}

// ---------------------------------------------------------------------------
// Console
// ---------------------------------------------------------------------------

/**
 * The operator console: state machine plus DOM presentation.
 *
 * Every mutation goes through {@link OperatorConsole#submit}, which is the only
 * place that builds a command envelope. That is deliberate - revision,
 * idempotency key, pending tracking, and stale handling are one code path rather
 * than a rule each call site has to remember.
 */
export class OperatorConsole {
  /**
   * @param {{document: Document, api: object, tokens: object, now?: Function, timers?: object, cryptoImpl?: Crypto}} options
   */
  constructor({ document: doc, api, tokens, now, timers, cryptoImpl }) {
    this.doc = doc;
    this.api = api;
    this.tokens = tokens;
    this.now = now ?? (() => Date.now());
    this.cryptoImpl = cryptoImpl;
    this.timers = timers ?? {
      setTimeout: (fn, ms) => setTimeout(fn, ms),
      clearTimeout: (handle) => clearTimeout(handle),
    };

    /** @type {object|null} */
    this.snapshot = null;
    this.instanceId = null;
    this.stateRevision = null;
    this.eventSequence = null;
    this.capabilities = null;
    this.version = null;
    this.logs = [];
    this.worktrees = [];
    this.worktreeError = null;
    this.notifications = [];
    /** Pending command intents, keyed by intent ID. */
    this.pending = new Map();
    /** Disclosure state survives re-render so an open row does not snap shut. */
    this.expanded = new Set();

    this.transport = 'connecting';
    this.lastConfirmedAt = null;
    this.authRequired = false;
    this.abortController = null;
    this.reconnectAttempts = 0;
    this.reconnectHandle = null;
    this.pollHandle = null;
    this.notificationSeq = 0;
    this.logFilter = 'all';
    this.confirmResolver = null;
    this.confirmInvoker = null;
    /** What to re-focus after a re-render destroys the focused control. */
    this.focusHint = null;

    this.el = this.collectElements();
    this.bindEvents();
  }

  // -- element wiring -------------------------------------------------------

  /** @private */
  collectElements() {
    const byId = (id) => this.doc.getElementById(id);
    return {
      instanceSummary: byId('instance-summary'),
      connectionState: byId('connection-state'),
      connectionMark: byId('connection-mark'),
      connectionText: byId('connection-text'),
      connectionDetail: byId('connection-detail'),
      refresh: byId('btn-refresh'),
      disconnect: byId('btn-disconnect'),
      authSection: byId('auth-section'),
      authForm: byId('auth-form'),
      authToken: byId('auth-token'),
      authToggle: byId('auth-token-toggle'),
      authError: byId('auth-error'),
      statusMode: byId('status-mode'),
      statusActive: byId('status-active'),
      statusAttention: byId('status-attention'),
      statusProgress: byId('status-progress'),
      attentionSummary: byId('attention-summary'),
      attentionText: byId('attention-text'),
      lifecycleActions: byId('lifecycle-actions'),
      lifecycleHint: byId('lifecycle-hint'),
      tablist: byId('tablist'),
      changesGroups: byId('changes-groups'),
      changesPlaceholder: byId('changes-placeholder'),
      worktreesList: byId('worktrees-list'),
      worktreesPlaceholder: byId('worktrees-placeholder'),
      logList: byId('log-list'),
      logsPlaceholder: byId('logs-placeholder'),
      logLevel: byId('log-level'),
      logCount: byId('log-count'),
      notificationList: byId('notification-list'),
      notificationsPlaceholder: byId('notifications-placeholder'),
      livePolite: byId('live-polite'),
      liveAssertive: byId('live-assertive'),
      dialog: byId('confirm-dialog'),
      dialogTitle: byId('confirm-title'),
      dialogBody: byId('confirm-body'),
      dialogTarget: byId('confirm-target'),
      dialogCancel: byId('confirm-cancel'),
      dialogAccept: byId('confirm-accept'),
    };
  }

  /** @private */
  bindEvents() {
    if (this.el.refresh) {
      this.el.refresh.addEventListener('click', () => {
        void this.bootstrap();
      });
    }
    if (this.el.disconnect) {
      this.el.disconnect.addEventListener('click', () => this.disconnect());
    }
    if (this.el.authForm) {
      this.el.authForm.addEventListener('submit', (event) => {
        event.preventDefault();
        void this.submitToken();
      });
    }
    if (this.el.authToggle) {
      this.el.authToggle.addEventListener('click', () => this.toggleTokenVisibility());
    }
    if (this.el.logLevel) {
      this.el.logLevel.addEventListener('change', (event) => {
        this.logFilter = event.target.value;
        this.renderLogs();
      });
    }

    const tabs = this.tabs();
    for (const tab of tabs) {
      tab.addEventListener('click', () => this.selectTab(tab.id));
      tab.addEventListener('keydown', (event) => this.onTabKeydown(event, tabs));
    }

    if (this.el.dialogCancel) {
      this.el.dialogCancel.addEventListener('click', () => this.settleConfirm(false));
    }
    if (this.el.dialogAccept) {
      this.el.dialogAccept.addEventListener('click', () => this.settleConfirm(true));
    }
    if (this.el.dialog) {
      // Escape closes a native dialog without a click, so cancellation has to be
      // settled from the dialog's own events rather than only from the button.
      this.el.dialog.addEventListener('cancel', (event) => {
        event.preventDefault();
        this.settleConfirm(false);
      });
      this.el.dialog.addEventListener('close', () => this.settleConfirm(false));
    }
  }

  /** @private */
  tabs() {
    if (!this.el.tablist) return [];
    return Array.from(this.el.tablist.querySelectorAll('[role="tab"]'));
  }

  // -- lifecycle ------------------------------------------------------------

  /**
   * Read health, capabilities, and a coherent snapshot, then start streaming.
   *
   * Safe to call again at any time: it is also the recovery path for a replay
   * gap, a process-incarnation change, and the manual refresh button.
   *
   * @returns {Promise<boolean>}
   */
  async bootstrap() {
    this.stopStream();
    this.stopPolling();
    this.transport = 'connecting';
    this.render();

    try {
      const health = await this.api.health();
      this.version = health?.version ?? null;
    } catch (error) {
      this.transport = 'offline';
      this.reportFailure('Cannot reach this Conflux instance.', error);
      this.render();
      return false;
    }

    try {
      this.capabilities = await this.api.capabilities();
      const state = await this.api.state();
      this.adoptState(state);
      this.authRequired = false;
    } catch (error) {
      if (error instanceof ApiFailure && error.errorCode === 'unauthorized') {
        this.requireAuthentication(error);
        return false;
      }
      this.transport = 'offline';
      this.reportFailure('The instance refused the initial state request.', error);
      this.render();
      return false;
    }

    await this.refreshLogs();
    await this.refreshWorktrees();
    this.render();
    this.startStream();
    return true;
  }

  /**
   * Adopt a coherent `/api/v2/state` response as the trusted snapshot.
   * @private
   */
  adoptState(state) {
    this.instanceId = state.instance_id;
    this.stateRevision = state.state_revision;
    this.eventSequence = state.event_sequence;
    this.snapshot = state.snapshot;
    this.lastConfirmedAt = this.now();
  }

  /** Fetch the retained log ring. */
  async refreshLogs() {
    try {
      const logs = await this.api.logs();
      this.logs = Array.isArray(logs?.logs) ? logs.logs : [];
    } catch (error) {
      if (error instanceof ApiFailure && error.errorCode === 'unauthorized') {
        this.requireAuthentication(error);
        return;
      }
      this.reportFailure('Logs could not be read.', error);
    }
    this.renderLogs();
  }

  /** Fetch current worktrees, keeping the refusal reason when there is one. */
  async refreshWorktrees() {
    try {
      const response = await this.api.worktrees();
      this.worktrees = Array.isArray(response?.worktrees) ? response.worktrees : [];
      this.worktreeError = null;
    } catch (error) {
      this.worktrees = [];
      if (error instanceof ApiFailure && error.errorCode === 'unauthorized') {
        this.requireAuthentication(error);
        return;
      }
      this.worktreeError = error instanceof ApiFailure ? error : null;
    }
    this.renderWorktrees();
  }

  /**
   * Present the token form and stop touching protected resources.
   * @private
   */
  requireAuthentication(error) {
    this.authRequired = true;
    this.transport = 'offline';
    this.snapshot = null;
    this.logs = [];
    this.worktrees = [];
    this.stopStream();
    this.stopPolling();
    if (this.el.authError && error) {
      this.el.authError.textContent = `${error.message} (${error.errorCode})`;
    }
    this.render();
    if (this.el.authToken) this.el.authToken.focus();
  }

  /** @private */
  async submitToken() {
    const value = this.el.authToken ? String(this.el.authToken.value ?? '').trim() : '';
    if (!value) {
      if (this.el.authError) this.el.authError.textContent = 'Enter the API token to connect.';
      if (this.el.authToken) this.el.authToken.focus();
      return;
    }
    this.tokens.set(value);
    if (this.el.authError) this.el.authError.textContent = '';
    const ok = await this.bootstrap();
    if (ok) {
      // The value only ever lived in the field and the token store; clearing the
      // field keeps it out of autofill and out of a screenshot.
      if (this.el.authToken) this.el.authToken.value = '';
      this.announce('Connected.', 'polite');
      if (this.el.refresh) this.el.refresh.focus();
    } else {
      this.tokens.clear();
    }
  }

  /** @private */
  toggleTokenVisibility() {
    const input = this.el.authToken;
    const toggle = this.el.authToggle;
    if (!input || !toggle) return;
    const show = input.type === 'password';
    input.type = show ? 'text' : 'password';
    toggle.setAttribute('aria-pressed', show ? 'true' : 'false');
    toggle.textContent = show ? 'Hide' : 'Show';
  }

  /** Drop every credential and every protected value this tab holds. */
  disconnect() {
    this.tokens.clear();
    this.stopStream();
    this.stopPolling();
    this.snapshot = null;
    this.logs = [];
    this.worktrees = [];
    this.worktreeError = null;
    this.capabilities = null;
    this.stateRevision = null;
    this.eventSequence = null;
    this.instanceId = null;
    this.pending.clear();
    this.lastConfirmedAt = null;
    this.transport = 'offline';
    this.authRequired = true;
    if (this.el.authToken) this.el.authToken.value = '';
    this.render();
    this.renderLogs();
    this.announce('Disconnected. Credentials cleared.', 'polite');
    if (this.el.authToken) this.el.authToken.focus();
  }

  // -- freshness ------------------------------------------------------------

  /**
   * How much the displayed state can be trusted right now.
   *
   * @returns {'fresh'|'reconnecting'|'stale'|'disconnected'}
   */
  freshness() {
    if (!this.snapshot || this.transport === 'offline') return 'disconnected';
    const age = this.lastConfirmedAt === null ? Infinity : this.now() - this.lastConfirmedAt;
    if (age > STALE_AFTER_MS) return 'stale';
    if (this.transport === 'connecting' || this.transport === 'reconnecting') return 'reconnecting';
    return 'fresh';
  }

  /** True when a command may be submitted from the current state. */
  canMutate() {
    return this.freshness() === 'fresh' && !this.authRequired && this.stateRevision !== null;
  }

  // -- event stream ---------------------------------------------------------

  /** @private */
  startStream() {
    this.stopStream();
    const controller = new AbortController();
    this.abortController = controller;
    this.streamTask = this.consumeStream(controller);
    void this.streamTask;
  }

  /** @private */
  async consumeStream(controller) {
    let stream;
    try {
      stream = await this.api.openEventStream({
        afterSequence: this.eventSequence,
        instanceId: this.instanceId,
        signal: controller.signal,
      });
    } catch (error) {
      if (controller.signal.aborted) return;
      if (error instanceof ApiFailure && error.errorCode === 'unauthorized') {
        this.requireAuthentication(error);
        return;
      }
      this.scheduleReconnect();
      return;
    }

    this.transport = 'stream';
    this.reconnectAttempts = 0;
    this.stopPolling();
    this.lastConfirmedAt = this.now();
    // Live observation is what makes the state trustworthy, so the whole view -
    // including whether mutation controls are enabled - is recomputed here.
    this.render();

    try {
      for await (const frame of stream.frames) {
        if (controller.signal.aborted) return;
        // Even a keep-alive comment proves the connection is alive, which is what
        // keeps a quiet instance from being reported as stale.
        this.markConfirmed();
        if (frame.comment || !frame.data) continue;
        let envelope;
        try {
          envelope = JSON.parse(frame.data);
        } catch {
          // A frame we cannot parse means the ordered feed is no longer
          // trustworthy; recover through a snapshot rather than guessing.
          await this.recoverThroughSnapshot('The event stream produced an unreadable frame.');
          return;
        }
        const recovered = await this.handleEvent(envelope);
        if (recovered) return;
      }
    } catch {
      if (controller.signal.aborted) return;
    }
    if (!controller.signal.aborted) this.scheduleReconnect();
  }

  /**
   * Apply one ordered envelope.
   *
   * @returns {Promise<boolean>} true when the caller must stop consuming because
   *   a full recovery has taken over.
   */
  async handleEvent(envelope) {
    if (envelope && envelope.instance_id && envelope.instance_id !== this.instanceId) {
      await this.recoverThroughSnapshot('The instance restarted, so state was reloaded.');
      return true;
    }
    if (envelope && envelope.category === 'gap') {
      await this.recoverThroughSnapshot(
        'Event history was no longer replayable, so state was reloaded.',
      );
      return true;
    }
    const sequence = Number(envelope?.event_sequence);
    if (!Number.isFinite(sequence)) {
      await this.recoverThroughSnapshot(
        'An event arrived without a usable sequence, so state was reloaded.',
      );
      return true;
    }
    if (this.eventSequence !== null && sequence <= this.eventSequence) return false;
    if (this.eventSequence !== null && sequence !== this.eventSequence + 1) {
      await this.recoverThroughSnapshot('Events arrived out of order, so state was reloaded.');
      return true;
    }
    this.eventSequence = sequence;
    this.markConfirmed();

    if (envelope.category === 'log') {
      if (envelope.payload && typeof envelope.payload === 'object') {
        this.logs.push(envelope.payload);
        if (this.logs.length > 1000) this.logs.shift();
        this.renderLogs();
      }
      return false;
    }

    // A state event only says *that* the projection moved. The snapshot itself
    // is re-read, so the console never reconstructs state from event payloads.
    await this.refreshSnapshot();
    await this.refreshWorktrees();
    return false;
  }

  /** @private */
  async refreshSnapshot() {
    try {
      const state = await this.api.state();
      this.adoptState(state);
      this.render();
    } catch (error) {
      if (error instanceof ApiFailure && error.errorCode === 'unauthorized') {
        this.requireAuthentication(error);
        return;
      }
      this.render();
    }
  }

  /** @private */
  async recoverThroughSnapshot(message) {
    this.stopStream();
    this.announce(message, 'polite');
    this.notify({ tone: 'info', title: 'State reloaded', body: message });
    const ok = await this.bootstrap();
    if (!ok) this.scheduleReconnect();
  }

  /** @private */
  markConfirmed() {
    this.lastConfirmedAt = this.now();
    this.renderConnection();
  }

  /** @private */
  stopStream() {
    if (this.abortController) {
      this.abortController.abort();
      this.abortController = null;
    }
    if (this.reconnectHandle !== null) {
      this.timers.clearTimeout(this.reconnectHandle);
      this.reconnectHandle = null;
    }
  }

  /** @private */
  scheduleReconnect() {
    this.reconnectAttempts += 1;
    if (this.reconnectAttempts > MAX_RECONNECT_ATTEMPTS) {
      // Streaming is not coming back on its own. Keep observing through no-store
      // snapshots and say plainly that live observation is degraded.
      this.transport = 'reconnecting';
      this.startPolling();
      this.render();
      return;
    }
    this.transport = 'reconnecting';
    this.render();
    const delay =
      RECONNECT_BACKOFF_MS[Math.min(this.reconnectAttempts - 1, RECONNECT_BACKOFF_MS.length - 1)];
    this.reconnectHandle = this.timers.setTimeout(() => {
      this.reconnectHandle = null;
      this.startStream();
    }, delay);
  }

  // -- polling fallback -----------------------------------------------------

  /** @private */
  startPolling() {
    if (this.pollHandle !== null) return;
    const tick = async () => {
      this.pollHandle = null;
      await this.pollOnce();
      if (this.transport === 'poll' || this.transport === 'reconnecting') {
        this.pollHandle = this.timers.setTimeout(tick, POLL_INTERVAL_MS);
      }
    };
    this.pollHandle = this.timers.setTimeout(tick, 0);
  }

  /** @private */
  stopPolling() {
    if (this.pollHandle !== null) {
      this.timers.clearTimeout(this.pollHandle);
      this.pollHandle = null;
    }
  }

  /** One no-store snapshot poll. Exposed so tests can drive the fallback. */
  async pollOnce() {
    try {
      const state = await this.api.state();
      if (state.instance_id !== this.instanceId) {
        await this.bootstrap();
        return;
      }
      this.adoptState(state);
      this.transport = 'poll';
      this.render();
    } catch (error) {
      if (error instanceof ApiFailure && error.errorCode === 'unauthorized') {
        this.requireAuthentication(error);
        return;
      }
      this.transport = 'reconnecting';
      this.render();
    }
  }

  // -- commands -------------------------------------------------------------

  /**
   * Submit one typed command against the latest confirmed revision.
   *
   * `intentId` identifies the user's intent, not the request: it is what makes a
   * second activation of the same control a no-op while the first is pending,
   * and what makes an outcome-unknown transport retry reuse the same key instead
   * of creating a second side effect.
   *
   * @param {{intentId: string, command: object, label?: string}} intent
   * @returns {Promise<object|null>} the settled command record, or null
   */
  async submit({ intentId, command, label }) {
    if (this.pending.has(intentId)) return null;
    if (!this.canMutate()) {
      this.notify({
        tone: 'error',
        title: 'Action refused',
        body: 'The displayed state is not current, so no command was sent. Refresh and try again.',
        recovery: 'Refresh the console, confirm the current state, then choose the action again.',
      });
      return null;
    }

    const envelope = {
      ...command,
      expected_revision: this.stateRevision,
      idempotency_key: newIdempotencyKey(this.cryptoImpl),
    };
    this.pending.set(intentId, envelope);
    this.render();

    try {
      const record = await this.sendWithUnknownOutcomeRetry(envelope);
      this.pending.delete(intentId);
      this.settleCommand(record, label);
      return record;
    } catch (error) {
      this.pending.delete(intentId);
      await this.handleCommandFailure(error, label);
      return null;
    } finally {
      this.pending.delete(intentId);
      this.render();
    }
  }

  /**
   * Send the envelope, retrying only when the transport left the outcome unknown.
   *
   * The retry reuses the *same* envelope, so the server's idempotency registry
   * collapses it onto the first attempt if that one landed.
   *
   * @private
   */
  async sendWithUnknownOutcomeRetry(envelope) {
    try {
      return await this.api.command(envelope);
    } catch (error) {
      if (error instanceof ApiFailure && error.outcomeUnknown) {
        return this.api.command(envelope);
      }
      throw error;
    }
  }

  /** @private */
  settleCommand(record, label) {
    const name = label ?? record?.type ?? 'Command';
    if (record?.state === 'failed') {
      this.notify({
        tone: 'error',
        title: `${name} failed`,
        body: record.detail ?? 'The instance rejected the command.',
        errorCode: record.error_code ?? null,
        correlationId: record.correlation_id ?? null,
        recovery: recoveryFor(record.error_code ?? ''),
      });
      this.announce(`${name} failed.`, 'assertive');
    } else if (record?.state === 'no_op') {
      this.notify({ tone: 'info', title: `${name} changed nothing`, body: record.detail ?? '' });
      this.announce(`${name} changed nothing.`, 'polite');
    } else if (record?.state === 'running') {
      this.notify({
        tone: 'info',
        title: `${name} accepted`,
        body: 'The command is still running.',
      });
      this.announce(`${name} accepted and still running.`, 'polite');
    } else {
      this.notify({ tone: 'success', title: `${name} succeeded`, body: record?.detail ?? '' });
      this.announce(`${name} succeeded.`, 'polite');
    }
    void this.refreshSnapshot();
  }

  /** @private */
  async handleCommandFailure(error, label) {
    const name = label ?? 'Command';
    if (error instanceof ApiFailure && error.errorCode === 'unauthorized') {
      this.requireAuthentication(error);
      return;
    }
    if (error instanceof ApiFailure && error.errorCode === 'stale_revision') {
      // Never replay the side effect against state the user has not seen: reload
      // and make them decide again.
      await this.refreshSnapshot();
      this.notify({
        tone: 'error',
        title: `${name} was not applied`,
        body: 'The state changed before the command was accepted. Review the refreshed state and choose again.',
        errorCode: 'stale_revision',
        correlationId: error.correlationId,
        recovery: recoveryFor('stale_revision'),
      });
      this.announce(`${name} was not applied because the state changed.`, 'assertive');
      return;
    }
    const failure = error instanceof ApiFailure ? error : null;
    this.notify({
      tone: 'error',
      title: `${name} failed`,
      body: failure?.message ?? String(error?.message ?? error),
      errorCode: failure?.errorCode ?? null,
      correlationId: failure?.correlationId ?? null,
      recovery: recoveryFor(failure?.errorCode ?? ''),
    });
    this.announce(`${name} failed.`, 'assertive');
  }

  // -- confirmation dialog --------------------------------------------------

  /**
   * Ask for confirmation before a destructive command.
   *
   * @param {{title: string, body: string, target?: string, confirmLabel?: string, invoker?: Element|null}} options
   * @returns {Promise<boolean>}
   */
  confirmDestructive({ title, body, target, confirmLabel, invoker }) {
    const dialog = this.el.dialog;
    if (!dialog) return Promise.resolve(false);
    if (this.confirmResolver) return Promise.resolve(false);

    if (this.el.dialogTitle) this.el.dialogTitle.textContent = title;
    if (this.el.dialogBody) this.el.dialogBody.textContent = body;
    if (this.el.dialogTarget) {
      this.el.dialogTarget.textContent = target ?? '';
      this.el.dialogTarget.hidden = !target;
    }
    if (this.el.dialogAccept) {
      this.el.dialogAccept.textContent = confirmLabel ?? 'Confirm';
      this.el.dialogAccept.disabled = false;
    }
    this.confirmInvoker = invoker ?? this.doc.activeElement ?? null;

    const promise = new Promise((resolve) => {
      this.confirmResolver = resolve;
    });
    if (typeof dialog.showModal === 'function') dialog.showModal();
    else dialog.setAttribute('open', '');
    // Cancel is the least destructive choice, so it is where focus lands.
    if (this.el.dialogCancel) this.el.dialogCancel.focus();
    return promise;
  }

  /** @private */
  settleConfirm(confirmed) {
    const resolve = this.confirmResolver;
    if (!resolve) return;
    this.confirmResolver = null;
    if (confirmed && this.el.dialogAccept) this.el.dialogAccept.disabled = true;
    const dialog = this.el.dialog;
    if (dialog) {
      if (typeof dialog.close === 'function' && dialog.open) dialog.close();
      else dialog.removeAttribute('open');
    }
    const invoker = this.confirmInvoker;
    this.confirmInvoker = null;
    if (invoker && typeof invoker.focus === 'function') invoker.focus();
    resolve(confirmed);
  }

  // -- notifications --------------------------------------------------------

  /**
   * Record a notification.
   *
   * Successes may be dismissed at leisure; anything the operator has to act on
   * stays until dismissed, because a toast that vanishes takes the correlation ID
   * with it.
   *
   * @param {{tone: string, title: string, body?: string, errorCode?: string|null, correlationId?: string|null, recovery?: string}} entry
   */
  notify(entry) {
    this.notificationSeq += 1;
    this.notifications.unshift({
      id: `n${this.notificationSeq}`,
      persistent: entry.tone === 'error',
      at: this.now(),
      ...entry,
    });
    this.notifications = this.notifications.slice(0, 50);
    this.renderNotifications();
  }

  /** Dismiss one notification by ID. */
  dismissNotification(id) {
    this.notifications = this.notifications.filter((item) => item.id !== id);
    this.renderNotifications();
  }

  /** @private */
  reportFailure(title, error) {
    const failure = error instanceof ApiFailure ? error : null;
    this.notify({
      tone: 'error',
      title,
      body: failure?.message ?? String(error?.message ?? error),
      errorCode: failure?.errorCode ?? null,
      correlationId: failure?.correlationId ?? null,
      recovery: recoveryFor(failure?.errorCode ?? ''),
    });
  }

  /**
   * Announce a status change to assistive technology.
   *
   * Routine updates are polite; only a failed mutation interrupts. Stream traffic
   * is never announced individually - that is what makes the region usable.
   *
   * @param {string} message
   * @param {'polite'|'assertive'} [tone]
   */
  announce(message, tone = 'polite') {
    const target = tone === 'assertive' ? this.el.liveAssertive : this.el.livePolite;
    if (target) target.textContent = message;
  }

  // -- tabs -----------------------------------------------------------------

  /** Activate a tab by element ID and show its panel. */
  selectTab(tabId) {
    for (const tab of this.tabs()) {
      const selected = tab.id === tabId;
      tab.setAttribute('aria-selected', selected ? 'true' : 'false');
      tab.tabIndex = selected ? 0 : -1;
      const panel = this.doc.getElementById(tab.getAttribute('aria-controls'));
      if (panel) panel.hidden = !selected;
    }
  }

  /** @private */
  onTabKeydown(event, tabs) {
    const index = tabs.indexOf(event.target);
    if (index === -1) return;
    let next = null;
    if (event.key === 'ArrowRight') next = tabs[(index + 1) % tabs.length];
    else if (event.key === 'ArrowLeft') next = tabs[(index - 1 + tabs.length) % tabs.length];
    else if (event.key === 'Home') next = tabs[0];
    else if (event.key === 'End') next = tabs[tabs.length - 1];
    if (!next) return;
    event.preventDefault();
    this.selectTab(next.id);
    next.focus();
  }

  // -- rendering ------------------------------------------------------------

  /** Re-render everything that depends on the snapshot. */
  render() {
    this.captureFocusHint();

    this.renderConnection();
    this.renderAuth();
    this.renderStatus();
    this.renderChanges();
    this.renderWorktrees();
    this.renderNotifications();

    this.restoreFocus();
  }

  /**
   * Remember what the focused control *is* before its node is destroyed.
   *
   * Rows are rebuilt from scratch on every state update, so keeping the element
   * is useless; the intent it carries is stable across renders and is what lets
   * keyboard focus survive a live update.
   *
   * @private
   */
  captureFocusHint() {
    const active = this.doc.activeElement;
    if (!active || active === this.doc.body) return;
    if (active.dataset?.intent) {
      this.focusHint = { attribute: 'data-intent', value: active.dataset.intent };
    } else if (active.dataset?.disclosureFor) {
      this.focusHint = { attribute: 'data-disclosure-for', value: active.dataset.disclosureFor };
    } else {
      this.focusHint = null;
    }
  }

  /** @private */
  restoreFocus() {
    const hint = this.focusHint;
    if (!hint) return;
    // Something already holds focus, so nothing was lost.
    if (this.doc.activeElement && this.doc.activeElement !== this.doc.body) {
      this.focusHint = null;
      return;
    }
    for (const element of this.doc.querySelectorAll(`[${hint.attribute}]`)) {
      const value =
        hint.attribute === 'data-intent' ? element.dataset.intent : element.dataset.disclosureFor;
      if (value !== hint.value) continue;
      // Still pending: keep the hint and land the focus once it is operable
      // again, rather than dropping the user at the top of the document.
      if (element.disabled) return;
      element.focus();
      this.focusHint = null;
      return;
    }
    this.focusHint = null;
  }

  renderConnection() {
    const freshness = this.freshness();
    const label = {
      fresh: this.transport === 'poll' ? 'Live (polling)' : 'Live',
      reconnecting: 'Reconnecting',
      stale: 'Stale',
      disconnected: this.authRequired ? 'Not authenticated' : 'Disconnected',
    }[freshness];

    setText(this.el.connectionText, label);
    if (this.el.connectionMark) {
      this.el.connectionMark.className = `connection-mark connection-mark-${freshness}`;
    }
    if (this.el.connectionState) this.el.connectionState.dataset.freshness = freshness;
    setText(
      this.el.connectionDetail,
      this.lastConfirmedAt === null
        ? 'Last confirmed update: never'
        : `Last confirmed update: ${formatTime(new Date(this.lastConfirmedAt))}`,
    );
    if (this.el.instanceSummary) {
      const parts = [];
      if (this.version) parts.push(this.version);
      if (this.instanceId) parts.push(`instance ${this.instanceId.slice(0, 12)}`);
      if (this.stateRevision !== null) parts.push(`revision ${this.stateRevision}`);
      this.el.instanceSummary.textContent = parts.length ? parts.join(' · ') : 'Not connected';
    }
    if (this.el.disconnect) this.el.disconnect.hidden = !this.tokens.get();
  }

  renderAuth() {
    if (this.el.authSection) this.el.authSection.hidden = !this.authRequired;
  }

  renderStatus() {
    const snapshot = this.snapshot;
    const totals = snapshot?.totals ?? null;
    const grouped = groupChanges(snapshot?.changes ?? []);

    setText(this.el.statusMode, snapshot ? describeMode(snapshot) : 'Unknown');
    setText(
      this.el.statusActive,
      snapshot
        ? grouped.active.length === 0
          ? 'Nothing running'
          : `${grouped.active.length} change${grouped.active.length === 1 ? '' : 's'} running`
        : 'Unknown',
    );
    setText(
      this.el.statusAttention,
      snapshot
        ? grouped.attention.length === 0
          ? 'Nothing'
          : `${grouped.attention.length} change${grouped.attention.length === 1 ? '' : 's'}`
        : 'Unknown',
    );
    setText(
      this.el.statusProgress,
      totals
        ? `${totals.completed} of ${totals.total} complete, ${totals.pending} waiting`
        : 'Unknown',
    );

    if (this.el.attentionSummary && this.el.attentionText) {
      const has = grouped.attention.length > 0;
      this.el.attentionSummary.hidden = !has;
      if (has) {
        const names = grouped.attention.slice(0, 3).map((change) => change.id);
        const more = grouped.attention.length - names.length;
        this.el.attentionText.textContent =
          `${grouped.attention.length} change${grouped.attention.length === 1 ? '' : 's'} need attention: ` +
          names.join(', ') +
          (more > 0 ? `, and ${more} more` : '') +
          '. Open the Changes view to recover.';
      }
    }

    this.renderLifecycleActions();
  }

  renderLifecycleActions() {
    const host = this.el.lifecycleActions;
    if (!host) return;
    clear(host);

    if (!this.snapshot) {
      const message = this.doc.createElement('p');
      message.className = 'hint';
      message.textContent = this.authRequired
        ? 'Authenticate to see the available actions.'
        : 'Actions become available once current state is confirmed.';
      host.appendChild(message);
      this.renderLifecycleHint();
      return;
    }

    for (const action of lifecycleActions(this.snapshot)) {
      const button = this.doc.createElement('button');
      button.type = 'button';
      button.id = `action-${action.id}`;
      button.dataset.intent = action.id;
      button.className = `btn ${
        action.primary ? 'btn-primary' : action.destructive ? 'btn-danger' : 'btn-secondary'
      }`;
      button.textContent = this.pending.has(action.id) ? `${action.label}…` : action.label;
      button.disabled = !this.canMutate() || this.pending.has(action.id);
      if (this.pending.has(action.id)) button.setAttribute('aria-busy', 'true');
      if (action.description) button.title = action.description;
      button.addEventListener('click', () => void this.invokeLifecycle(action, button));
      host.appendChild(button);
    }
    this.renderLifecycleHint();
  }

  /** @private */
  renderLifecycleHint() {
    const hint = this.el.lifecycleHint;
    if (!hint) return;
    const freshness = this.freshness();
    if (this.authRequired) {
      hint.hidden = false;
      hint.textContent = 'Actions are unavailable until this tab is authenticated.';
      return;
    }
    if (freshness === 'fresh') {
      hint.hidden = true;
      hint.textContent = '';
      return;
    }
    hint.hidden = false;
    hint.textContent =
      freshness === 'disconnected'
        ? 'Actions are unavailable while the console is disconnected.'
        : 'Actions are unavailable until current state is confirmed again.';
  }

  /** @private */
  async invokeLifecycle(action, button) {
    if (action.destructive) {
      const confirmed = await this.confirmDestructive({
        title: `${action.label}?`,
        body: action.description ?? 'This action cannot be undone.',
        confirmLabel: action.label,
        invoker: button,
      });
      if (!confirmed) return;
    }
    await this.submit({ intentId: action.id, command: action.command, label: action.label });
  }

  renderChanges() {
    const host = this.el.changesGroups;
    if (!host) return;
    clear(host);

    const changes = this.snapshot?.changes ?? [];
    if (this.el.changesPlaceholder) {
      this.el.changesPlaceholder.hidden = changes.length > 0;
      this.el.changesPlaceholder.textContent = this.snapshot
        ? 'No changes are being tracked by this instance.'
        : this.authRequired
          ? 'Authenticate to see changes.'
          : 'Loading changes…';
    }
    if (changes.length === 0) return;

    const grouped = groupChanges(changes);
    for (const group of CHANGE_GROUPS) {
      const items = grouped[group.key];
      if (items.length === 0) continue;
      const section = this.doc.createElement('section');
      section.className = `change-group change-group-${group.key}`;
      section.dataset.group = group.key;
      const heading = this.doc.createElement('h4');
      heading.textContent = `${group.title} (${items.length})`;
      section.appendChild(heading);
      const list = this.doc.createElement('ul');
      list.className = 'resource-list';
      for (const change of items) list.appendChild(this.renderChangeRow(change, group.key));
      section.appendChild(list);
      host.appendChild(section);
    }
  }

  /** @private */
  renderChangeRow(change, groupKey) {
    const item = this.doc.createElement('li');
    item.className = 'resource';
    item.dataset.changeId = change.id;
    item.dataset.group = groupKey;

    const head = this.doc.createElement('div');
    head.className = 'resource-head';
    const title = this.doc.createElement('span');
    title.className = 'resource-title';
    title.setAttribute('translate', 'no');
    title.textContent = change.id;
    head.appendChild(title);
    head.appendChild(this.renderStatusBadge(change.display_status, groupKey));
    item.appendChild(head);

    const percent = Math.round(Number(change.progress_percent ?? 0));
    const progress = this.doc.createElement('p');
    progress.className = 'resource-progress';
    progress.textContent = `${change.completed_tasks ?? 0} of ${change.total_tasks ?? 0} tasks (${percent}%)`;
    item.appendChild(progress);

    const meter = this.doc.createElement('div');
    meter.className = 'meter';
    meter.setAttribute('role', 'progressbar');
    meter.setAttribute('aria-valuemin', '0');
    meter.setAttribute('aria-valuemax', '100');
    meter.setAttribute('aria-valuenow', String(percent));
    meter.setAttribute('aria-valuetext', `${percent}% of tasks complete`);
    meter.setAttribute('aria-label', `Task progress for ${change.id}`);
    const fill = this.doc.createElement('span');
    fill.className = 'meter-fill';
    fill.style.width = `${percent}%`;
    meter.appendChild(fill);
    item.appendChild(meter);

    const details = this.doc.createElement('div');
    details.className = 'resource-details';
    details.id = `details-${cssId(change.id)}`;
    details.hidden = !this.expanded.has(change.id);

    const actions = this.doc.createElement('div');
    actions.className = 'resource-actions';
    for (const action of this.changeActions(change, groupKey)) {
      actions.appendChild(this.renderChangeAction(change, action));
    }

    const disclosure = this.doc.createElement('button');
    disclosure.type = 'button';
    disclosure.className = 'btn btn-secondary disclosure';
    disclosure.dataset.disclosureFor = change.id;
    disclosure.setAttribute('aria-expanded', this.expanded.has(change.id) ? 'true' : 'false');
    disclosure.setAttribute('aria-controls', details.id);
    disclosure.textContent = `Details for ${change.id}`;
    disclosure.addEventListener('click', () => {
      const open = this.expanded.has(change.id);
      if (open) this.expanded.delete(change.id);
      else this.expanded.add(change.id);
      disclosure.setAttribute('aria-expanded', open ? 'false' : 'true');
      details.hidden = open;
    });
    actions.appendChild(disclosure);
    item.appendChild(actions);

    const list = this.doc.createElement('dl');
    appendDetail(this.doc, list, 'Status', change.display_status ?? 'unknown');
    appendDetail(this.doc, list, 'Task status', change.progress_status ?? 'unknown');
    if (change.iteration_number !== undefined && change.iteration_number !== null) {
      appendDetail(this.doc, list, 'Iteration', String(change.iteration_number));
    }
    appendDetail(
      this.doc,
      list,
      'Dependencies',
      change.dependencies?.length ? change.dependencies.join(', ') : 'None',
    );
    details.appendChild(list);
    item.appendChild(details);

    return item;
  }

  /** @private */
  renderStatusBadge(status, groupKey) {
    const badge = this.doc.createElement('span');
    badge.className = `badge badge-${groupKey}`;
    const mark = this.doc.createElement('span');
    mark.className = 'badge-mark';
    mark.setAttribute('aria-hidden', 'true');
    // Shape carries the same meaning as colour, so status never depends on hue.
    mark.textContent = { attention: '!', active: '▶', waiting: '○', completed: '✓' }[
      groupKey
    ];
    badge.appendChild(mark);
    const text = this.doc.createElement('span');
    text.textContent = status ?? 'unknown';
    badge.appendChild(text);
    return badge;
  }

  /**
   * Contextual actions for a change, derived from its current v2 status.
   * @private
   */
  changeActions(change, groupKey) {
    const status = String(change.display_status ?? '').toLowerCase();
    const actions = [];
    if (status === 'error' || status === 'stalled') {
      actions.push({
        id: `retry-${change.id}`,
        label: 'Retry',
        command: { type: 'retry_change', change_id: change.id },
      });
    }
    if (status === 'merge wait') {
      actions.push({
        id: `resolve-${change.id}`,
        label: 'Resolve merge',
        command: { type: 'resolve_merge', change_id: change.id },
      });
    }
    if (groupKey === 'active') {
      actions.push({
        id: `stop-${change.id}`,
        label: 'Stop and dequeue',
        command: { type: 'stop_and_dequeue', change_id: change.id },
        destructive: true,
        consequence: `Stopping ${change.id} terminates its running work and removes it from the queue.`,
      });
    }
    if (status === 'not queued') {
      actions.push({
        id: `queue-${change.id}`,
        label: 'Add to queue',
        command: { type: 'set_queue_intent', change_id: change.id, queued: true },
      });
    } else if (status === 'queued') {
      actions.push({
        id: `dequeue-${change.id}`,
        label: 'Remove from queue',
        command: { type: 'set_queue_intent', change_id: change.id, queued: false },
      });
    }
    return actions;
  }

  /** @private */
  renderChangeAction(change, action) {
    const button = this.doc.createElement('button');
    button.type = 'button';
    button.className = `btn ${action.destructive ? 'btn-danger' : 'btn-secondary'}`;
    button.dataset.intent = action.id;
    button.textContent = this.pending.has(action.id) ? `${action.label}…` : action.label;
    button.setAttribute('aria-label', `${action.label}: ${change.id}`);
    button.disabled = !this.canMutate() || this.pending.has(action.id);
    if (this.pending.has(action.id)) button.setAttribute('aria-busy', 'true');
    button.addEventListener('click', async () => {
      if (action.destructive) {
        const confirmed = await this.confirmDestructive({
          title: `${action.label}?`,
          body: action.consequence ?? 'This action cannot be undone.',
          target: change.id,
          confirmLabel: action.label,
          invoker: button,
        });
        if (!confirmed) return;
      }
      await this.submit({
        intentId: action.id,
        command: action.command,
        label: `${action.label} ${change.id}`,
      });
    });
    return button;
  }

  renderWorktrees() {
    const host = this.el.worktreesList;
    const placeholder = this.el.worktreesPlaceholder;
    if (!host) return;
    clear(host);

    if (placeholder) {
      if (this.worktreeError) {
        placeholder.hidden = false;
        placeholder.textContent = `Worktrees are unavailable: ${this.worktreeError.message} (${this.worktreeError.errorCode})`;
      } else if (this.worktrees.length === 0) {
        placeholder.hidden = false;
        placeholder.textContent = this.authRequired
          ? 'Authenticate to see worktrees.'
          : 'No worktrees are present.';
      } else {
        placeholder.hidden = true;
      }
    }
    if (this.worktrees.length === 0) return;

    for (const worktree of this.worktrees) host.appendChild(this.renderWorktreeRow(worktree));
  }

  /** @private */
  renderWorktreeRow(worktree) {
    const item = this.doc.createElement('li');
    item.className = 'resource';
    item.dataset.worktreeId = worktree.worktree_id;

    const head = this.doc.createElement('div');
    head.className = 'resource-head';
    const title = this.doc.createElement('span');
    title.className = 'resource-title';
    title.setAttribute('translate', 'no');
    title.textContent = worktree.branch || '(detached HEAD)';
    head.appendChild(title);
    if (worktree.is_main) {
      const badge = this.doc.createElement('span');
      badge.className = 'badge badge-waiting';
      badge.textContent = 'main worktree';
      head.appendChild(badge);
    }
    item.appendChild(head);

    const path = this.doc.createElement('p');
    path.className = 'resource-path';
    path.setAttribute('translate', 'no');
    path.textContent = worktree.path;
    item.appendChild(path);

    const facts = this.doc.createElement('dl');
    facts.className = 'resource-details-inline';
    appendDetail(this.doc, facts, 'HEAD', worktree.head ?? 'unknown');
    appendDetail(
      this.doc,
      facts,
      'Uncommitted changes',
      worktree.dirty === null || worktree.dirty === undefined
        ? 'Could not be determined'
        : worktree.dirty
          ? 'Yes'
          : 'No',
    );
    appendDetail(
      this.doc,
      facts,
      'Commits ahead of base',
      worktree.has_commits_ahead ? 'Yes' : 'No',
    );
    if (worktree.conflict) {
      appendDetail(
        this.doc,
        facts,
        'Conflict',
        `${worktree.conflict.files.join(', ')} — recovery: ${worktree.conflict.recovery}. Resolve locally or in the TUI.`,
      );
    }
    item.appendChild(facts);

    const operations = worktree.operations ?? {};
    const actions = this.doc.createElement('div');
    actions.className = 'resource-actions';
    actions.appendChild(
      this.renderWorktreeAction({
        worktree,
        enabled: operations.mergeable === true,
        label: 'Merge',
        command: {
          type: 'merge_worktree',
          target: { worktree_id: worktree.worktree_id },
          params: {},
        },
        intentId: `merge-${worktree.worktree_id}`,
        destructive: false,
        consequence: `Merging ${worktree.branch || worktree.path} into base preserves any conflict for local or TUI recovery.`,
      }),
    );
    actions.appendChild(
      this.renderWorktreeAction({
        worktree,
        enabled: operations.deletable === true,
        label: 'Delete',
        command: {
          type: 'delete_worktree',
          target: { worktree_id: worktree.worktree_id },
          params: {},
        },
        intentId: `delete-${worktree.worktree_id}`,
        destructive: true,
        consequence:
          'Deleting this worktree runs managed teardown and removes its working directory. This cannot be undone.',
      }),
    );
    item.appendChild(actions);

    for (const [operation, reason] of [
      ['Merge', operations.merge_blocked_reason],
      ['Delete', operations.delete_blocked_reason],
    ]) {
      if (!reason) continue;
      const blocked = this.doc.createElement('p');
      blocked.className = 'blocked-reason';
      blocked.dataset.operation = operation.toLowerCase();
      blocked.textContent = `${operation} unavailable: ${reason}`;
      item.appendChild(blocked);
    }

    return item;
  }

  /** @private */
  renderWorktreeAction({ worktree, enabled, label, command, intentId, destructive, consequence }) {
    const button = this.doc.createElement('button');
    button.type = 'button';
    button.className = `btn ${destructive ? 'btn-danger' : 'btn-secondary'}`;
    button.dataset.intent = intentId;
    button.textContent = this.pending.has(intentId) ? `${label}…` : label;
    button.setAttribute('aria-label', `${label} worktree ${worktree.branch || worktree.path}`);
    // Eligibility is the server's answer, never inferred from dirty/ahead here.
    button.disabled = !enabled || !this.canMutate() || this.pending.has(intentId);
    if (this.pending.has(intentId)) button.setAttribute('aria-busy', 'true');
    button.addEventListener('click', async () => {
      const confirmed = await this.confirmDestructive({
        title: `${label} this worktree?`,
        body: consequence,
        target: worktree.branch || worktree.path,
        confirmLabel: label,
        invoker: button,
      });
      if (!confirmed) return;
      await this.submit({ intentId, command, label: `${label} worktree` });
    });
    return button;
  }

  renderLogs() {
    const host = this.el.logList;
    if (!host) return;
    clear(host);

    const minimum = LOG_LEVEL_ORDER[this.logFilter] ?? 0;
    const visible = this.logs.filter((entry) => {
      if (this.logFilter === 'all') return true;
      const level = LOG_LEVEL_ORDER[String(entry?.level ?? 'info')] ?? 0;
      return level >= minimum;
    });

    if (this.el.logsPlaceholder) {
      this.el.logsPlaceholder.hidden = visible.length > 0;
      this.el.logsPlaceholder.textContent = this.logs.length
        ? 'No log entries match the selected level.'
        : 'No log entries yet.';
    }
    setText(this.el.logCount, `${visible.length} of ${this.logs.length} entries`);

    for (const entry of visible.slice(-500)) {
      const item = this.doc.createElement('li');
      item.className = `log-entry log-${String(entry?.level ?? 'info')}`;

      const meta = this.doc.createElement('span');
      meta.className = 'log-meta';
      const parts = [entry?.timestamp ?? '', String(entry?.level ?? 'info').toUpperCase()];
      if (entry?.change_id) parts.push(entry.change_id);
      meta.textContent = parts.filter(Boolean).join(' ');
      item.appendChild(meta);

      const message = this.doc.createElement('span');
      message.className = 'log-message';
      // Log content becomes text nodes only; markup in a message stays literal.
      message.appendChild(renderAnsi(this.doc, entry?.message ?? ''));
      item.appendChild(message);

      host.appendChild(item);
    }
  }

  renderNotifications() {
    const host = this.el.notificationList;
    if (!host) return;
    clear(host);

    if (this.el.notificationsPlaceholder) {
      this.el.notificationsPlaceholder.hidden = this.notifications.length > 0;
    }

    for (const entry of this.notifications) {
      const item = this.doc.createElement('li');
      item.className = `notification notification-${entry.tone}`;
      item.dataset.tone = entry.tone;

      const title = this.doc.createElement('p');
      title.className = 'notification-title';
      const mark = this.doc.createElement('span');
      mark.className = 'notification-mark';
      mark.setAttribute('aria-hidden', 'true');
      mark.textContent = { success: '✓', info: 'i', error: '!' }[entry.tone] ?? 'i';
      title.appendChild(mark);
      title.appendChild(this.doc.createTextNode(entry.title));
      item.appendChild(title);

      if (entry.body) {
        const body = this.doc.createElement('p');
        body.className = 'notification-body';
        body.textContent = entry.body;
        item.appendChild(body);
      }
      if (entry.recovery) {
        const recovery = this.doc.createElement('p');
        recovery.className = 'notification-recovery';
        recovery.textContent = `Next step: ${entry.recovery}`;
        item.appendChild(recovery);
      }
      if (entry.errorCode || entry.correlationId) {
        const meta = this.doc.createElement('p');
        meta.className = 'notification-meta';
        meta.setAttribute('translate', 'no');
        const bits = [];
        if (entry.errorCode) bits.push(`code ${entry.errorCode}`);
        if (entry.correlationId) bits.push(`correlation ${entry.correlationId}`);
        meta.textContent = bits.join(' · ');
        item.appendChild(meta);
      }

      const dismiss = this.doc.createElement('button');
      dismiss.type = 'button';
      dismiss.className = 'btn btn-secondary';
      dismiss.textContent = 'Dismiss';
      dismiss.setAttribute('aria-label', `Dismiss notification: ${entry.title}`);
      dismiss.addEventListener('click', () => this.dismissNotification(entry.id));
      item.appendChild(dismiss);

      host.appendChild(item);
    }
  }
}

// ---------------------------------------------------------------------------
// DOM utilities
// ---------------------------------------------------------------------------

function clear(node) {
  while (node.firstChild) node.removeChild(node.firstChild);
}

function setText(node, value) {
  if (node) node.textContent = value;
}

function appendDetail(doc, list, term, value) {
  const wrapper = doc.createElement('div');
  const dt = doc.createElement('dt');
  dt.textContent = term;
  const dd = doc.createElement('dd');
  dd.textContent = value;
  wrapper.appendChild(dt);
  wrapper.appendChild(dd);
  list.appendChild(wrapper);
}

function cssId(value) {
  return String(value).replace(/[^A-Za-z0-9_-]/g, '-');
}

function formatTime(date) {
  try {
    return new Intl.DateTimeFormat(undefined, { timeStyle: 'medium' }).format(date);
  } catch {
    return date.toISOString();
  }
}

// ---------------------------------------------------------------------------
// Startup
// ---------------------------------------------------------------------------

/**
 * Build and start a console against the current document.
 *
 * @param {object} [options]
 * @returns {OperatorConsole}
 */
export function start(options = {}) {
  const doc = options.document ?? globalThis.document;
  const tokens = options.tokens ?? createTokenStore(safeSessionStorage());
  const api = options.api ?? createApiClient({ tokens, fetchImpl: options.fetchImpl });
  const instance = new OperatorConsole({ ...options, document: doc, api, tokens });
  void instance.bootstrap();
  return instance;
}

function safeSessionStorage() {
  try {
    return globalThis.sessionStorage ?? null;
  } catch {
    return null;
  }
}

if (typeof document !== 'undefined' && !globalThis.__CFLX_NO_AUTOSTART__) {
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => start());
  } else {
    start();
  }
}
