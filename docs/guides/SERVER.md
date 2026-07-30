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

## Web UI and Dashboard

The Web UI is a monitoring dashboard backed by HTTP and WebSocket state. It works in both normal mode (`--web`) and server mode.

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

### Dashboard Features

- Dashboard UI at `http://localhost:<port>/`
- Real-time updates over WebSocket
- REST API for querying state
- QR code popup in the TUI via `w`

### REST API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/health` | GET | Health check |
| `/api/state` | GET | Full orchestrator state |
| `/api/changes` | GET | List all changes with progress |
| `/api/changes/{id}` | GET | Details for a specific change |

For complete API details, see [../openapi.yaml](../openapi.yaml).

### WebSocket

Connect to `ws://localhost:<port>/ws` for real-time state updates.

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

### Web UI Troubleshooting

| Issue | Solution |
|-------|----------|
| Address already in use | Use `--web-port 0` or choose an unused port |
| Dashboard not loading | Confirm `--web` is enabled and the URL has the correct port |
| WebSocket disconnects frequently | Check network stability; the dashboard auto-reconnects |
| Changes not updating | Refresh the page or confirm the orchestrator is running |
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
