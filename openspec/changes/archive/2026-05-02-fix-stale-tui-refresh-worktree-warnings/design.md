# Design: Stale TUI Auto-Refresh Root Handling

## Current Behavior

The local TUI auto-refresh task captures `repo_root` at startup and runs read-only snapshot commands every refresh tick. If that path is later removed, `tokio::process::Command` fails while setting or using `current_dir`, and the runner logs three warning classes every tick.

This is noisy because the warning volume is caused by stale UI process state, not by repeated independent workflow failures.

## Desired Behavior

The refresh loop should treat a missing or invalid captured root as a stale refresh source. Stale refresh sources should be reported once or with bounded backoff, then skipped until the session ends or an explicit future reload path replaces the root.

## Implementation Shape

Preferred minimal implementation:

1. Add a small helper in the TUI refresh path that checks root usability before the snapshot command group.
2. Track a session-local boolean or timestamp for whether the stale-root warning has already been emitted.
3. If the root is missing/invalid, emit the bounded stale-root warning and continue the loop without running VCS snapshot commands.
4. Keep the existing per-command warning behavior for roots that exist.

The warning-bound state is UI/session-local observability state only. It must not influence workflow routing, acceptance, archive, rejection, or resume decisions.

## Alternatives Considered

- Silencing all refresh snapshot errors: rejected because real git failures with existing roots remain actionable.
- Automatically recreating or switching worktrees: rejected as too broad and potentially conflicting with workspace-local workflow state rules.
- Treating missing root as fatal TUI error: rejected because this is a stale refresh source and should not necessarily force app error mode.
