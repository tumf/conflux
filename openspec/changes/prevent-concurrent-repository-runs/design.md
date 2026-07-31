# Design: Repository-Scoped Process Lock

## Decision

Use one non-blocking advisory lock keyed by the canonical Git common directory and retain the locked file descriptor for the lifetime of a local orchestration process.

The lock file lives under the Git common directory rather than the current worktree. This makes the base checkout and linked worktrees share one exclusion domain while unrelated repositories remain independent.

## Startup Flow

1. Classify the invocation as local orchestration or a bypassed command.
2. Resolve the repository's canonical Git common directory.
3. Open the repository lock file and attempt a non-blocking exclusive OS lock.
4. On success, write initial diagnostic metadata and retain the lock owner at the outer entrypoint scope.
5. Start optional Web/API listeners and other runtime components.
6. When a listener returns its actual accessible URL, atomically refresh metadata with the API base URL.
7. On conflict, read metadata best-effort, report valid fields, and exit before side effects.

## Ownership and Metadata

The kernel lock is the only ownership authority. The JSON metadata is diagnostic output and may be absent, incomplete, malformed, or left over from a previous process. None of those states can block a new process when the OS lock is available.

Suggested metadata fields:

```json
{
  "pid": 1234,
  "started_at": "2026-07-31T12:00:00Z",
  "workspace": "/canonical/repository",
  "mode": "run",
  "api_url": "http://127.0.0.1:39876/api/v2"
}
```

Initial metadata omits `api_url`. Listener startup updates it only after bind succeeds, including when port `0` produces an OS-assigned port. Metadata replacement must avoid exposing partially written JSON to a competitor.

## Entry Point Policy

Lock acquisition applies to:

- `cflx` default local TUI
- `cflx tui` local mode
- `cflx run`
- `cflx server`

It does not apply to:

- TUI remote-client mode connected through `--server`
- completion, logs, validation, inspection, installation, and other non-orchestration commands

Dry-run remains guarded because it shares startup and repository analysis paths and should not overlap an active repository owner without an explicit safe concurrency contract.

## Failure Behavior

A live conflict returns a non-zero status and a concise diagnostic. When metadata is valid, include PID, mode, start time, workspace, and API URL. Missing or malformed optional fields are omitted rather than causing startup failure or claiming stale ownership.

No `--force`, PID probing, stale-file deletion, or timeout-based lock stealing is provided. Operators stop the owning process or use its reported API endpoint.

## Platform Scope

The initial implementation uses the already available Unix system interface on macOS and Linux. A Windows implementation is future work and must preserve the same RAII and non-blocking behavior before Windows is considered supported for this feature.

## Constitutional Fit

The lock prevents concurrent mutation but does not decide workflow phase, completion, acceptance, archive readiness, or merge eligibility. Metadata is observability-only and therefore does not violate workspace-local workflow state requirements.
