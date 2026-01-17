# Change: Archive/Merge Guardrails to Prevent Archived Change Resurrection

## Why

After `openspec archive` completes, changes sometimes reappear in the `openspec/changes/` directory, causing inconsistencies where execution stops at `MergeWait`. This change strengthens archive completion detection and pre-merge verification to detect and prevent reverse movement from archive back to changes.

## What Changes

- Add existence check for `openspec/changes/<change_id>` to archive commit completion detection.
- Fail archive commit creation phase if `openspec/changes/<change_id>` still exists.
- Re-verify `verify_archive_completion` before merge execution; defer to `MergeWait` if not archived.

## Impact

- Affected specs: parallel-execution
- Affected code: `src/execution/archive.rs`, `src/parallel/mod.rs`
