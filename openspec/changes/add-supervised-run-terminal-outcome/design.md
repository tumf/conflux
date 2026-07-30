# Design: supervised run terminal outcome

## Context

Non-interactive run currently shares an outer retry loop with optional web controls. On orchestrator error it publishes lifecycle `blocked` and waits indefinitely for retry or stop. Lifecycle delivery is deliberately non-blocking and lossy, so it cannot prove that a one-shot job completed.

## Decisions

### Decision: opt-in one-shot mode, not a global run rewrite

`--supervised` selects a separate terminal policy at the run entrypoint. Ordinary run keeps its current stdout logs and operator/web retry loop. The supervised path executes one orchestrator attempt, performs bounded cleanup, emits one result, and exits.

This keeps compatibility and avoids silently changing local operator behavior.

### Decision: stdout is a result channel

In supervised mode stdout contains one compact JSON object and a trailing newline. All logs and diagnostics use stderr. The record is emitted once from one owner after orchestration and lifecycle cleanup facts are known.

The writer does not print a fallback second record. Serialization is covered exhaustively because the schema contains only serializable owned fields. A stdout write failure produces a non-zero process status; the supervisor treats absence of a valid record as an abnormal failure.

### Decision: typed outcome propagation

Human-readable error strings and lifecycle states are not classifiers. Core orchestration returns typed terminal data to the run entrypoint. The upstream finalizer's scheduler outcome and remote-confirmation result are consumed directly.

Outcome meanings:

- `completed`: all selected work reached its required terminal success, including remote confirmation when upstream integration is enabled;
- `blocked`: no safe runnable progress exists because repository-visible dependency/manual gating requires intervention;
- `stalled`: attempted execution reached a resumable bounded hold such as acceptance stall or unresolved repository repair;
- `cancelled`: graceful operator signal or typed cancellation stopped the run;
- `failed`: fatal configuration, authentication, verification, push, command, invariant, or non-resumable error.

`resumable` is derived from the typed repository/workspace outcome, not inferred from exit code or prose. Exit code 2 means a supervisor may offer explicit retry, not that completion occurred.

### Decision: result data is privacy-limited and non-authoritative

The schema includes public identifiers and observed SHAs, but no arbitrary errors, command output, environment, credentials, prompts, or config. Failures expose an enum-like `reason_code` and an optional bounded sanitized summary.

The record is an authoritative observation of this process attempt for the supervisor, but never a workflow-control input for cflx. Restart routing is recomputed from the persistent checkout.

### Decision: lifecycle remains unchanged

The existing adapter continues to receive process and coarse semantic state events. It may omit or drop them without changing the result. No new guarantee is layered onto its queue or shutdown deadline.

## Record Shape

```json
{
  "schema_version": 1,
  "type": "run_terminal",
  "outcome": "completed",
  "resumable": false,
  "reason_code": null,
  "remote": "origin",
  "branch": "main",
  "local_head": "<sha>",
  "remote_head": "<sha>",
  "selected_changes": ["change-a", "change-b"],
  "processed_changes": ["change-b"],
  "already_completed_changes": ["change-a"],
  "pending_changes": []
}
```

Optional identity fields are omitted when unavailable. Change arrays are always present, deduplicated, and retain requested order where applicable.

## Exit Mapping

| Outcome | Exit code |
|---|---:|
| `completed` | 0 |
| `blocked` | 2 |
| `stalled` | 2 |
| `cancelled` | 3 |
| `failed` | 1 |

## Failure Boundaries

- Clap rejection before supervised mode initializes follows clap's existing process contract and may have no terminal record.
- Controlled startup failure after the option is recognized emits `failed` when the result channel is available.
- SIGTERM/SIGINT uses bounded graceful cancellation and emits `cancelled`.
- SIGKILL, abort, runtime panic, container kill after grace, or record-channel failure may leave no valid record; the supervisor combines that absence with process/container status.
