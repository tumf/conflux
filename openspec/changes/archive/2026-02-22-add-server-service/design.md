## Context

Conflux already provides `cflx server` (multi-project server daemon) which is intentionally directory-independent and loads only global configuration.
This change adds a thin, OS-specific service management layer that launches and supervises `cflx server`.

## Goals / Non-Goals

- Goals:
  - Provide `install/uninstall/status/start/stop/restart` for `cflx server`.
  - Make the implementation safe-by-default (validate server security policy before starting).
  - Keep service definitions minimal and deterministic.
- Non-Goals:
  - Implement a full service manager abstraction beyond what is needed for these subcommands.
  - Store secrets inside service definition files.

## Decisions

- Decision: Implement `cflx service` as a CLI subcommand group.
  - Why: aligns with existing CLI patterns and keeps `cflx server` unchanged.

- Decision: Linux default uses systemd user services.
  - Why: allows background execution without root on most modern distros.

- Decision: Validate `ServerConfig` (including non-loopback auth enforcement) before start/restart.
  - Why: prevents accidental insecure service configuration.

## Risks / Trade-offs

- OS command availability differs (e.g., `systemctl --user`, `launchctl`, `schtasks`).
  - Mitigation: fail-fast with actionable error messages; keep generation functions testable.

## Migration Plan

1. Users update to a version that includes `cflx service`.
2. Users configure global `server` settings (and bearer token if binding non-loopback).
3. Users run `cflx service install` then `cflx service start`.

## Open Questions

- Should OpenRC be supported in the initial release, or deferred to a follow-up change?
