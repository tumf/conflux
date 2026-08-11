# Design: Preserve archiving during TUI refresh

## Root Cause

The refresh path observes two facts with different authority:

1. The reducer owns the active lifecycle and reports `archiving` after `ArchiveStarted`.
2. Repository inspection can already report an archive-complete workspace and place the change in `ChangesRefreshed.merge_wait_ids`.

The event dispatcher applies `ChangesRefreshed` to the reducer before the TUI receives it. `OrchestratorState::apply_observation` preserves active state, so the synchronized reducer snapshot still says `archiving`. The TUI then handles the same refresh locally and applies `merge wait` because its protected-status helper lists selected active states but omits `archiving`.

The false status is therefore introduced after authoritative synchronization, inside presentation-only refresh handling.

## State Precedence

For refresh-derived archived-workspace evidence, display precedence remains:

1. reducer-owned terminal/error state;
2. reducer-owned active state;
3. reducer-owned pending or manual wait state;
4. explicit stop/dequeue state;
5. refresh-derived display correction.

`merge_wait_ids` proves archive completion without base integration. It does not by itself prove that Archive has returned or that an operator must intervene.

## Minimal Change

Use the existing `orchestration::operator_command::is_active_status` predicate in the TUI refresh protection decision, then keep the current checks for protected non-active states.

This removes the duplicate active-status subset without introducing a new abstraction. The shared predicate already contains the canonical display vocabulary used by command and lifecycle code.

The refresh detector remains unchanged because it is still needed for fresh-process recovery and stale display correction. `OrchestratorState::apply_observation` remains unchanged because it already refuses to overwrite active execution.

## Verification Design

### Event-order regression

Construct an `OrchestratorState` and `AppState` for one change, apply `ArchiveStarted`, synchronize reducer display caches, then handle `ChangesRefreshed` with the change in `merge_wait_ids`. Assert:

- reducer status is `archiving`;
- TUI status is `archiving`;
- queue intent and execution marks do not change.

### Active vocabulary coverage

Exercise each status recognized by `is_active_status` against refresh merge-wait evidence and assert it remains unchanged. This binds the TUI precedence rule to the shared vocabulary rather than to a test-local duplicate list.

### Compatibility coverage

Keep or extend existing tests proving:

- reducer-owned `resolve pending` and `resolving` remain protected;
- terminal/error and explicit `not queued` remain protected;
- stale display-only pending may still be corrected;
- a fresh process may restore merge wait from archived workspace evidence;
- concrete manual deferral remains merge wait.

## Risks

- Over-protecting every reducer status would disable startup restoration. The change protects only shared active statuses plus the already protected pending/terminal/stop states.
- Changing archive detection would hide useful repository evidence and break resume behavior. Detection is intentionally unchanged.
- `on_workspace_status_merge_wait` has a narrower active-state guard, but production emits `WorkspaceStatus::MergeWait` only for concrete post-archive manual deferral. Changing that reducer path is outside this TUI refresh fix.
- The refresh helper's existing terminal list intentionally remains unchanged because its omitted statuses cannot coincide with archived-but-not-integrated refresh evidence; broad terminal-vocabulary cleanup is outside this fix.
- Creating another active-status table would repeat the root cause. The implementation must expose and reuse the existing classifier's backing vocabulary for exhaustive coverage.
