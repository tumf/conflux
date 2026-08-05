# Web UI Guide

This guide covers the default local API socket, the optional web monitoring UI
(`--web`), and the versioned `/api/v2` remote-control API.

Conflux is a single-workspace tool: `cflx`, `cflx tui`, and `cflx run` all
operate on the current repository. The web UI is an optional monitoring surface
attached to that one process — there is no standalone daemon and no
multi-project server.

> **Removed:** the obsolete multi-project server product (`cflx server`,
> `cflx project`, `cflx service`, and the remote-client TUI `--server` options)
> no longer exists. See [Migrating from server mode](#migrating-from-server-mode).

## The default local API socket

`cflx`, `cflx tui`, and `cflx run` serve `/api/v2` on a Unix domain socket by
default. No flag enables it and no TCP port is consumed:

```
${GIT_COMMON_DIR}/cflx-api.sock
```

```bash
# Query the running process from a script, an agent, or another shell
SOCK="$(git rev-parse --git-common-dir)/cflx-api.sock"
curl --unix-socket "$SOCK" http://localhost/api/v2/health
curl --unix-socket "$SOCK" http://localhost/api/v2/state

# With a bearer token configured
curl --unix-socket "$SOCK" -H "Authorization: Bearer $CFLX_WEB_TOKEN" \
  http://localhost/api/v2/state
```

The host part of those URLs is ignored by the server; `curl` needs *some*
authority, so `localhost` is conventional.

The path comes from the canonical Git *common* directory — the same repository
identity the orchestration lock uses. Every linked worktree of one repository
therefore resolves the same socket, unrelated repositories resolve different
ones, and the lock is what keeps two default owners from racing for it.

| Option | Effect |
|--------|--------|
| *(none)* | Bind `${GIT_COMMON_DIR}/cflx-api.sock` |
| `--web-unix-socket PATH` | Bind `PATH` instead |
| `--no-web-unix-socket` | Bind no Unix socket at all |

The two options are mutually exclusive. Outside a Git repository there is no
identity to derive a deterministic path from, so startup fails with an error
naming both explicit choices rather than guessing a location.

**Permissions and authentication.** The socket is created with mode `0600`, so
filesystem permissions are the access boundary. Token-free access is permitted
there exactly as it is on loopback TCP. When a bearer token *is* configured, one
policy applies to every active listener: `/api/v2/health` stays public and every
other v2 HTTP, SSE, and WebSocket resource requires it.

**Startup and shutdown.** The listener binds before lifecycle adapters, AI
subprocesses, or orchestration begin. A bind, permission, or path-safety failure
exits non-zero with none of that started. A live socket or a non-socket entry at
the target path is never removed — startup fails and leaves it alone — while an
unreachable socket left by a dead process is replaced. A finite `cflx run` and a
graceful TUI exit both remove the socket they created, and never a replacement
that appeared at the path during the run.

**Browsers and QR codes.** A `unix://` endpoint is discovery information for
local clients and reverse proxies. Browsers cannot open it, and the TUI QR popup
still encodes the TCP Web UI URL only. To expose the API over HTTP, front the
socket with a reverse proxy (nginx `proxy_pass http://unix:/path/to/cflx-api.sock:/`,
Caddy `reverse_proxy unix//path/to/cflx-api.sock`) or use `--web`.

## Enabling the Web UI

`--web` *adds* the browser-facing TCP listener; it does not replace or disable
the Unix socket. Both listeners serve the same router and the same process
state.

```bash
# TUI + Web UI (plus the default Unix socket)
cflx --web

# Headless run + Web UI
cflx run --web

# Custom port, plus a bearer token because the bind is not loopback
export CFLX_WEB_TOKEN="$(openssl rand -hex 32)"
cflx --web --web-port 9000 --web-bind 0.0.0.0 --web-auth-token-env CFLX_WEB_TOKEN

# TCP only, with no Unix socket
cflx run --web --no-web-unix-socket
```

When using the default port (`0`), the OS automatically assigns an available port.
The bound address is logged when the server starts.

A non-loopback `--web-bind` requires a bearer token for the `/api/v2`
remote-control API, and the process refuses to start without one. Use
`--web-auth-token-env VAR` (recommended) or `--web-auth-token TOKEN`; the two are
mutually exclusive. See [USAGE.md](USAGE.md#remote-control-api-apiv2).

`--web-port`, `--web-bind`, and the allowed-origin options configure the TCP
listener only; the Unix socket is controlled solely by its default, override, or
opt-out.

## Dashboard Features

- Operator console at `http://localhost:<port>/`
- Real-time updates over the `/api/v2` event stream
- Versioned REST API for querying state and submitting commands
- QR code popup in the TUI via `w`

## REST API Endpoints

The console is a pure `/api/v2` client: every read, event, and mutation goes
through the versioned remote-control contract. There is no unversioned `/api/*`
or `/ws` surface.

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/v2/health` | GET | Health check |
| `/api/v2/state` | GET | Full orchestrator state |
| `/api/v2/changes` | GET | List all changes with progress |
| `/api/v2/changes/{id}` | GET | Details for a specific change |
| `/api/v2/commands` | POST | Submit a control command |

See [USAGE.md](USAGE.md#remote-control-api-apiv2) for authentication, revisions,
and idempotency.

### Worktree deletion is fail-closed here

`delete_worktree` always runs managed teardown and refuses any worktree that has
uncommitted changes, has commits ahead of base, or whose safety state could not
be observed. Such a worktree is listed with `operations.deletable: false` and a
`delete_blocked_reason`, and the reason never discloses an absolute path.

The command object accepts no parameters at all: `force`, `skip_teardown`,
`allow_known_dirty`, `allow_commits_ahead`, `path`, and `branch` are schema
violations (HTTP 422) rather than ignored fields, and no request ever reaches the
service. Discarding uncommitted changes or unmerged commits is a local decision
taken at the TUI's uppercase-`X` confirmation — see
[USAGE.md](USAGE.md#deleting-a-worktree-from-the-tui). Neither this API nor the
operator console exposes an equivalent.

For complete API details, see [../openapi.yaml](../openapi.yaml).

### The state resource is authoritative

`GET /api/v2/state` is a complete replacement for whatever a client currently
holds, not a summary of it. Every change carries, in addition to its
reducer-derived `display_status`:

- `execution_marked` and `queue_intent` as separate fields
- `attention` (`new` for a change detected after the process started watching)
- `blocker` with a machine-readable `kind` (`dependency`, `external`, or `none`
  for a stalled execution hold) and sanitized detail
- `error_detail` for a change-local failure, kept apart from the snapshot-level
  `process_error`
- `actions`, stating for every change-addressed command whether it is allowed
  and, when it is not, a stable `blocked_reason` token
- `parallel` eligibility with a stable `blocked_reason`
- `timing` boundaries, `latest_activity`, and the repository-relative `worktree`
  relation

Absent values are explicit `null`s rather than omitted keys, so a client that
replaces its local data from one snapshot can also *clear* a field. Nothing here
is durable: execution marks, attention, and timing all start empty in a new
process, and workflow routing is recomputed from the workspace.

## Event stream

Connect to `/api/v2/events` (SSE) or `ws://localhost:<port>/api/v2/ws` for
real-time state updates.

```json
{
  "type": "state_update",
  "timestamp": "2024-01-12T10:30:00Z",
  "changes": [
    {
      "id": "add-feature",
      "completed_tasks": 3,
      "total_tasks": 10,
      "progress_percent": 30.0,
      "status": "in_progress"
    }
  ]
}
```

## Troubleshooting

| Issue | Solution |
|-------|----------|
| Address already in use | Use `--web-port 0` or choose an unused port |
| Dashboard not loading | Confirm `--web` is enabled and the URL has the correct port |
| WebSocket disconnects frequently | Check network stability; the dashboard auto-reconnects |
| Changes not updating | Refresh the page or confirm the orchestrator is running |
| Cannot access from another device | Use `--web-bind 0.0.0.0` for local network access (also requires `--web-auth-token-env`) |

## Migrating from server mode

The multi-project server product was removed. `cflx server`, `cflx project`,
`cflx service`, and the TUI `--server` / `--server-token` / `--server-token-env`
options now fail as usage errors.

Upgrading does not stop, uninstall, or delete anything you own. If you had
previously run `cflx service install`, stop and remove the service manually
**before or after** upgrading:

```bash
# macOS (launchd user agent)
launchctl bootout gui/$(id -u)/com.conflux.cflx-server 2>/dev/null || true
rm -f ~/Library/LaunchAgents/com.conflux.cflx-server.plist

# Linux (systemd user service)
systemctl --user disable --now cflx-server.service 2>/dev/null || true
rm -f ~/.config/systemd/user/cflx-server.service
systemctl --user daemon-reload

# Windows (Scheduled Task)
schtasks /Delete /TN cflx-server /F
```

Old server data (the projects registry and its database) is left untouched under
`${XDG_DATA_HOME:-~/.local/share}/cflx/server`. Nothing reads it any more, so
archive or delete that directory once you have confirmed you no longer need it.

A legacy `server` section left in a global config file is simply ignored; the
rest of the configuration keeps working unchanged.
