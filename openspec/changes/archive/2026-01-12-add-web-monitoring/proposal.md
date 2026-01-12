# Change: Add Web Monitoring Interface

## Why
Currently, users can only monitor orchestration progress through the TUI or log files. This requires them to stay in the terminal or parse JSON state files. Adding an optional HTTP server with a web interface will enable:
- Remote monitoring of orchestration status from any device with a browser
- Easier visualization of progress, tasks, and change dependencies
- Better support for team collaboration and CI/CD integration

## What Changes
- Add an optional HTTP server that serves orchestration state via REST API and WebSocket
- Create a web-based dashboard UI for viewing:
  - Active changes and their progress
  - Task completion status
  - Real-time updates via WebSocket
  - Change dependency graph visualization
- Add CLI flag `--web` or `--http` to enable the web server
- Make the HTTP server port configurable (default: 8080)
- Support both run mode and TUI mode with web monitoring

## Impact
- Affected specs: `cli`, `configuration`
- New spec: `web-monitoring`
- Affected code:
  - `src/cli.rs` - Add HTTP server flags
  - `src/main.rs` - Initialize HTTP server when flag is set
  - New module: `src/web.rs` - HTTP server implementation
  - New directory: `web/` - Frontend assets (HTML/CSS/JS)
- No breaking changes - feature is opt-in via CLI flag
