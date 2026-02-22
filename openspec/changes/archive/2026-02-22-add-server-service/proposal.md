# Change: Install `cflx server` as a Background Service

## Why
Running `cflx server` as an interactive foreground process is fragile (terminal closure, user logout) and makes it harder to run a remote TUI against a stable endpoint.

## What Changes
- Add a new `cflx service` command group to manage `cflx server` as a background service.
- Provide cross-platform implementations:
  - macOS: launchd user agent
  - Linux: systemd user service (default)
  - Windows: Scheduled Task
- Enforce the existing server-mode security policy before starting (non-loopback bind requires bearer token).

## Impact
- Affected specs: `openspec/specs/cli/spec.md`, `openspec/specs/server-mode/spec.md`
- Affected code: `src/cli.rs`, `src/main.rs`, new `src/service/mod.rs` (and any platform helpers)
- Compatibility: additive CLI surface; existing commands remain unchanged

## Out of Scope
- Privileged/system-wide Linux services (e.g., systemd system units) beyond the minimum required to support common user-level background execution.
- OpenRC support unless explicitly required for a target environment.
