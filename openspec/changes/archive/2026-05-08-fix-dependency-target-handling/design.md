# Design: Fix Dependency Target Handling

## Current Failure Modes

Conflux currently has multiple dependency interpretations:

- `openspec.rs` parses proposal metadata dependencies into `Change.dependencies`.
- `analyzer.rs` can return empty dependencies for single-change fast paths and can reject archived dependency IDs because analyzer validation only allows queued order IDs and in-flight IDs.
- `parallel_run_service.rs` fallback analysis returns an empty dependency map when LLM analysis fails or is unavailable.
- `queue_state.rs` dispatch gating consults `AnalysisResult.dependencies`, so any dropped dependency can allow premature apply.
- `openspec_cmd.rs` already distinguishes archived dependencies as warnings, creating a mismatch with analyzer/scheduler behavior.

## Target Model

Dependency target classification should be repository-visible and deterministic:

- `Queued`: active `openspec/changes/<id>/proposal.md` exists and is part of the current queued analysis set.
- `InFlight`: dependency ID is explicitly reported by the scheduler as currently executing.
- `Archived`: `openspec/changes/archive/<id>/proposal.md` or date-prefixed `openspec/changes/archive/<date>-<id>/proposal.md` exists, or the base branch archive tree proves the dependency is merged.
- `Missing`: no queued, in-flight, or archived evidence exists.

The authoritative dependency edge source is proposal metadata/body parsing (`Change.dependencies`). LLM analysis may add valid required dependencies, but it must not remove metadata dependencies.

## Scheduler Semantics

- Queued and in-flight dependency targets are hard gates until `is_merged_to_base(dep_id, ...)` reports true.
- Archived dependency targets are satisfied and should not block dispatch.
- Missing dependency targets are fail-closed: the dependent change is not dispatched and a dedicated diagnostic is emitted.

## Analyzer Semantics

Analyzer validation should validate dependency references against queued IDs, in-flight IDs, and archived IDs. Archived IDs are allowed so analysis does not collapse them into parse/json failure. Missing IDs remain invalid and should include classification context.

## Constitution Alignment

The solution must not introduce durable workflow-control state outside the workspace or base git state. Classification must be derived from active proposal files, in-flight IDs supplied by the scheduler, archive directories, and base-branch tree checks.
