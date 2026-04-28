# Tasks

## Implementation

1. [x] Replace the bare-task warning preview slice in `src/openspec_cmd.rs` with a UTF-8-safe truncation path (verification: unit - `src/openspec_cmd.rs` no longer uses `&trimmed[..trimmed.len().min(50)]` for bare-task preview rendering)

2. [x] Add regression coverage for a bare task containing a multi-byte character at the previous byte cutoff boundary (verification: unit - add/update `src/openspec_cmd.rs` tests, then run `cargo test openspec_cmd`)

3. [x] Preserve the existing validation warning behavior for long bare tasks after the truncation change (verification: unit - `cargo test openspec_cmd` passes with assertions for `Possible task without checkbox` output)

4. [x] Update validation spec coverage for UTF-8-safe bare-task previews (verification: unit - `openspec/changes/fix-utf8-task-preview-slicing/specs/cflx-proposal-validation/spec.md` captures the behavior and `cflx openspec validate fix-utf8-task-preview-slicing --strict` passes)

## Acceptance #1 Failure Follow-up
- [x] Archive commit readiness is no longer blocked by dashboard/dist cleanliness. `dashboard/dist/index.html` references `/dashboard/assets/index-Bd3Kf0Z0.js` and `/dashboard/assets/index-Blko_xDv.css`, and those exact files exist in `dashboard/dist/assets/` with no generated-file drift.
- [x] `git status --porcelain` is empty for the clean-working-tree acceptance check. `dashboard/dist/index.html:8-9` asset references now match the actual generated filenames in `dashboard/dist/assets/`, resolving the previously reported mismatch with old asset paths.
