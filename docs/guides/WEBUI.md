# Web UI Guide

This guide covers the local web monitoring UI (`--web`), its REST API, and the
versioned `/api/v2` remote-control API.

Conflux is a single-workspace tool: `cflx`, `cflx tui`, and `cflx run` all
operate on the current repository. The web UI is an optional monitoring surface
attached to that one process — there is no standalone daemon and no
multi-project server.

> **Removed:** the obsolete multi-project server product (`cflx server`,
> `cflx project`, `cflx service`, and the remote-client TUI `--server` options)
> no longer exists. See [Migrating from server mode](#migrating-from-server-mode).

## Enabling the Web UI

```bash
# TUI + Web UI
cflx --web

# Headless run + Web UI
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

For complete API details, see [../openapi.yaml](../openapi.yaml).

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
