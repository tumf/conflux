# Design: archived dependency references in analyze

## Current behavior

`src/analyzer.rs` validates dependency edges by requiring every dependency target to be present in the queued `order` list or in the in-flight set. It can detect that a rejected dependency is archived only after constructing the error, but the result is still a parse failure.

This conflicts with the canonical requirement that archived dependency references must not collapse into generic JSON/parse failure behavior.

## Target behavior

Analyze validation should classify each dependency target before graph validation:

- queued: keep as dependency edge
- in-flight: keep as dependency edge for gating semantics
- archived: treat as already satisfied/non-queued and remove from executable dependency edges
- missing: fail with dedicated invalid dependency reference diagnostics

The normalized dependency graph should be the graph used for cycle detection and scheduling. Archived dependencies cannot participate in runtime ordering because they are already outside the active queue.

## Compatibility

This preserves existing behavior for queued, in-flight, and missing dependencies. It only changes archived dependency references from terminal parse failure into explicit satisfied/non-queued references, matching the existing canonical spec allowance.

## Verification approach

Unit tests should construct `AnalysisResult` values or parse JSON output with dependencies that reference archived directories under `openspec/changes/archive/`. The archived case must succeed and the missing case must fail.
