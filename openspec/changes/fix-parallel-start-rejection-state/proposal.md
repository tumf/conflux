# Change Proposal: fix-parallel-start-rejection-state

## Problem / Context

- Parallel execution uses an authoritative Git-based eligibility filter at start time to exclude changes that are not present in `HEAD` or that have uncommitted files under `openspec/changes/<change_id>/`.
- TUI and CLI callers can reach that start path with stale assumptions from earlier refresh state or selection state.
- In TUI, this can leave a row displayed as `Queued` after `F5` even though the backend rejected the change and scheduled nothing.
- In `cflx run --parallel`, the same backend rejection path can warn and then return early without making it obvious that zero changes actually started.
- The result is one underlying start-time reconciliation bug with two user-visible symptoms: stale queued state in TUI and ambiguous no-op completion in CLI.

## Proposed Solution

- Treat backend parallel-start filtering as the authoritative source of truth for both TUI and CLI start flows.
- Add explicit rejection reconciliation so any change filtered out during parallel start is reported back to the caller with enough information to restore a consistent non-running state.
- Ensure TUI start and stopped-mode resume paths clear `Queued` for rejected changes and explain why execution did not begin.
- Ensure `cflx run --parallel` makes it clear when backend filtering rejects all requested changes and no work actually starts.
- Add regression coverage for both the TUI stale-eligibility case and the CLI all-rejected case.

## Acceptance Criteria

- If a change becomes uncommitted after the last TUI refresh but before parallel start, the TUI does not leave that row in `Queued` after backend rejection.
- If parallel start rejects all requested changes, `cflx run --parallel` clearly reports that no changes started because they were filtered out as uncommitted or otherwise ineligible.
- TUI `F5` start and stopped-mode resume stay behaviorally aligned with the backend eligibility filter used by parallel execution.
- Regression tests cover both the TUI stale-state symptom and the CLI all-rejected symptom.

## Out of Scope

- Redefining parallel eligibility rules.
- Changing dependency analysis, dispatch ordering, or slot scheduling beyond start-time reconciliation and user-visible reporting.
- Changing workspace resume/state-detection behavior for already-created worktrees; that is covered by a separate proposal.
