# Server Mode Guide

This guide covers `cflx server`, remote TUI access, Web UI, REST API, and background service management.

## When to Use Server Mode

Use server mode when you want a long-lived daemon, multi-project management, remote APIs, or server-managed proposal sessions.

```bash
cflx server
```

Server mode exposes the Web UI and APIs for connected clients. A TUI can connect to a remote server with `--server`.

Remote TUI key input is still handled by the local client. The default start/resume/retry/continue key is `F5`; local clients can add bindings such as `r` in `~/.config/cflx/tui.jsonc` with `{"keybindings":{"start":["F5","r"]}}`. This file is a TUI-only user preference and is not read from project `.cflx.jsonc`.

## Web UI and Remote Monitoring

- In normal mode, enable the dashboard with `--web` on `cflx` or `cflx run`
- In server mode, the dashboard is part of the daemon setup
- Use normal mode for local monitoring and server mode for remote clients

```bash
# Local TUI + Web UI
cflx --web

# Local headless run + Web UI
cflx run --web

# Remote TUI connected to a server
cflx tui --server http://host:39876
```

## Server-Only Configuration

### Proposal Session `OPENCODE_CONFIG`

This setting applies to proposal sessions created by `cflx server`.
Server-side proposal sessions do not auto-generate or inject `OPENCODE_CONFIG`.
If `proposal_session.transport_env.OPENCODE_CONFIG` is not set, opencode uses its built-in default config.

To override the config, set `OPENCODE_CONFIG` explicitly:

```jsonc
{
  "proposal_session": {
    "transport_env": {
      "OPENCODE_CONFIG": "/absolute/path/to/opencode.json"
    }
  }
}
```

`OPENCODE_CONFIG` is optional and only needed when you want a custom opencode configuration file.

## Web UI: the operator console

The Web UI is an embedded operator console served as three static files
(`/`, `/style.css`, `/app.js`). It is a `/api/v2` client and nothing else: every
read, event, error, and mutation uses the versioned remote-control contract, so
the browser gets the same bearer authentication, optimistic revisions,
idempotency, and typed errors as any other controller. It works in both normal
mode (`--web`) and server mode.

### Enabling the Web UI

```bash
# Normal mode: TUI + Web UI
cflx --web

# Normal mode: headless run + Web UI
cflx run --web

# Custom port, plus a bearer token because the bind is not loopback
export CFLX_WEB_TOKEN="$(openssl rand -hex 32)"
cflx --web --web-port 9000 --web-bind 0.0.0.0 --web-auth-token-env CFLX_WEB_TOKEN
```

When using the default port (`0`), the OS automatically assigns an available port.
The bound address is logged when the server starts.

A non-loopback `--web-bind` requires a bearer token for the `/api/v2`
remote-control API, and the process refuses to start without one. Use
`--web-auth-token-env VAR` (recommended) or `--web-auth-token TOKEN`; the two are
mutually exclusive. See [USAGE.md](USAGE.md#remote-control-api-apiv2).

In server mode (`cflx server`), the Web UI is always available on the configured port.

### What the console shows

- Current status first: process identity, connection freshness, application
  mode, active work, anything needing attention, and the one valid next action.
- Changes grouped by operator priority — needs attention, active, waiting,
  completed — each with a labelled disclosure button for details.
- Worktrees addressed by their opaque `worktree_id`, with the server's own
  eligibility answer and its blocked reason when an operation is refused.
- A persistent log view with level filtering, and typed API errors that stay on
  screen with their `error_code`, correlation ID, and a recovery step.
- QR code popup in the TUI via `w`.

### Browser authentication

When the instance requires a token, the console shows a labelled token form
after its first unauthorized response. The token is sent only in the
`Authorization` header — never in a URL, a log line, a correlation ID, or
`localStorage`. A tab-scoped `sessionStorage` copy survives a reload and is
cleared by **Disconnect**, which also drops every protected value the tab holds.

### Event stream and recovery

The console reads `/api/v2/events` with `fetch()` response streaming and tracks
`instance_id` and `event_sequence`. A replay gap, a sequence discontinuity, an
unreadable frame, or a changed process incarnation makes it re-read
`/api/v2/state` before live observation resumes. If streaming stays unavailable
it falls back to no-store snapshot polling. Whenever the displayed state is not
current, the console says so — *reconnecting*, *stale*, or *disconnected* — and
refuses to submit commands until it is trusted again.

### Commands

Every mutation is a `POST /api/v2/commands` carrying the latest confirmed
`state_revision` and a per-intent idempotency key. A second activation while a
command is pending does nothing. A retry reuses the same key only when the
transport left the outcome unknown. A `stale_revision` response refreshes state
and asks for a new decision rather than replaying the side effect. Force stop,
stopping an active change, and deleting a worktree each require an accessible
confirmation dialog before any request is sent.

### API

`/api/v2` is the only HTTP contract. The legacy unversioned `/api/*` routes and
the browser `/ws` endpoint were removed once the console migrated; requests to
them return 404 and have no side effect. See
[USAGE.md](USAGE.md#remote-control-api-apiv2) for the endpoint reference and
[../openapi.yaml](../openapi.yaml) for the generated schema.

### Web UI Troubleshooting

| Issue | Solution |
|-------|----------|
| Address already in use | Use `--web-port 0` or choose an unused port |
| Console not loading | Confirm `--web` is enabled and the URL has the correct port |
| Stuck on "Authentication required" | The instance has a token configured; paste it into the token form |
| Connection shows "Stale" or "Disconnected" | Actions are disabled on purpose; use **Refresh now**, or check that the process is still running |
| Action refused with `stale_revision` | The state moved on; review the refreshed state and choose again |
| Cannot access from another device | Use `--web-bind 0.0.0.0` for local network access (also requires `--web-auth-token-env`) |

## Background Service (`cflx service`)

Use `cflx service` to install and manage `cflx server` as a user-level background service.

- macOS: `launchd` user agent
- Linux: `systemd --user` service
- Windows: Scheduled Task

```text
cflx service <install|uninstall|status|start|stop|restart>
```

Examples:

```bash
# Install and enable the service
cflx service install

# Start or restart the background server
cflx service start
cflx service restart

# Check service status
cflx service status

# Stop or remove the service
cflx service stop
cflx service uninstall
```

Notes:

- `install`, `start`, and `restart` validate the effective global `server` configuration before touching the service manager
- macOS writes a plist under `~/Library/LaunchAgents/com.conflux.cflx-server.plist`
- Linux writes a unit file under `~/.config/systemd/user/cflx-server.service`
- Configure persistent server settings in your global config before installing
