## Context

`cflx run` is a non-interactive command, but the current implementation shares control-path ideas with interactive/web-controlled execution. After a successful orchestration result, run mode enters a wait loop intended for restart/stop control rather than terminating the process. With `--web`, the process can also retain spawned lifecycle tasks unless shutdown is tied to successful completion.

## Goals / Non-Goals

- Goals:
  - Make successful `cflx run` completion terminate promptly.
  - Make successful `cflx run --web` completion terminate promptly.
  - Keep the fix narrow and avoid changing unrelated retry/error semantics.
- Non-Goals:
  - Rework run-mode error recovery UX.
  - Change TUI execution semantics.

## Decisions

- Decision: Treat successful orchestration completion as terminal for run mode.
  - Why: `cflx run` is a non-interactive command and should not require an additional stop signal after success.
- Decision: Explicitly tie run-scoped helper tasks to the run lifecycle.
  - Why: success should not depend on Tokio runtime teardown behavior or lingering background tasks.

## Risks / Trade-offs

- Removing the post-success wait loop may reduce opportunities for web-driven restart after a clean run.
  - Mitigation: keep this change scoped to success only; leave error/retry semantics unchanged.
- Web-monitoring cleanup may touch shared helper code.
  - Mitigation: constrain cleanup behavior to tasks started by run mode and cover with regression tests.

## Migration Plan

1. Update the run-mode success path to return/exit after logging completion.
2. Ensure signal/web helper tasks can be cancelled or dropped cleanly on success.
3. Add regression tests for non-web and web success paths.

## Open Questions

- None.
