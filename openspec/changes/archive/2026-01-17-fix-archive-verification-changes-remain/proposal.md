# Change: Archive verification fails when changes directory remains

## Why
Archive verification currently reports success even when `openspec/changes/{change_id}` still exists, which causes `ensure_archive_commit` to fail and surfaces errors in the TUI. This change ensures the verification fails in that case so the workflow does not continue while the change is still unarchived.

## What Changes
- `verify_archive_completion` treats a change as unarchived when `openspec/changes/{change_id}` exists, regardless of archive entries.
- The existing behavior that treats missing changes as successful remains unchanged.
- The parallel, serial, and TUI archive checks share the same decision logic.

## Impact
- Affected specs: `parallel-execution`, `cli`
- Affected code: `src/execution/archive.rs`, `src/parallel/executor.rs`, `src/tui/orchestrator.rs`, `src/orchestration/archive.rs`
