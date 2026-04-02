---
change_type: implementation
priority: medium
dependencies: []
references:
  - openspec/specs/server-mode/spec.md
  - openspec/specs/server-api/spec.md
  - openspec/specs/server-mode-dashboard/spec.md
  - openspec/specs/git-sync/spec.md
  - src/server/api.rs
  - src/server/registry.rs
  - src/remote/types.rs
---

# Change: Add remote sync-state monitoring for server projects

**Change Type**: implementation

## Problem / Context

Server mode can manually execute `git/sync`, but it does not continuously expose whether each registered project is currently ahead of, behind, diverged from, or equal to its configured remote branch. As a result, operators cannot quickly see which projects need synchronization before deciding whether to run sync.

This request is intentionally scoped to monitoring and visibility only. It does not add automatic synchronization or automatic reconciliation.

## Proposed Solution

- Add a periodic background remote-state check in `cflx server` for each registered project.
- Compute and retain display-ready sync state metadata for every project, including local/remote SHA, ahead count, behind count, sync state, and last check status.
- Expose that metadata through server state APIs and WebSocket full-state payloads used by the TUI and dashboard.
- Update server-mode dashboard and remote project views to display ahead/behind state clearly so users can judge whether sync is needed.
- Preserve the non-invasive behavior of monitoring: checks must not trigger `git/sync` or `resolve_command` automatically.

## Acceptance Criteria

- Registered server projects are periodically checked against their configured remote branch while `cflx server` is running.
- Each project has a computed sync state of `up_to_date`, `ahead`, `behind`, `diverged`, or `unknown`.
- Each project exposes `ahead_count`, `behind_count`, `local_sha`, `remote_sha`, `last_remote_check_at`, and check error information when available.
- The server state snapshot and WebSocket `full_state` include this sync-state metadata for all projects.
- The server-mode dashboard can render the sync state without inferring it from raw logs or ad hoc git commands.
- Periodic monitoring never triggers `git/sync` or `resolve_command` automatically.

## Out of Scope

- Automatic sync or push/pull execution
- Webhook-triggered monitoring
- Per-project custom polling intervals
- Auto-remediation for diverged or failed projects

## Impact

- Affected specs: `server-mode`, `server-api`, `server-mode-dashboard`
- Affected code: `src/server/api.rs`, `src/server/mod.rs`, `src/server/registry.rs`, `src/remote/types.rs`, dashboard/TUI remote state consumers
