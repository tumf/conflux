# Tasks

## Implementation

1. [x] Replace the bare-task warning preview slice in `src/openspec_cmd.rs` with a UTF-8-safe truncation path (verification: unit - `src/openspec_cmd.rs` no longer uses `&trimmed[..trimmed.len().min(50)]` for bare-task preview rendering)

2. [x] Add regression coverage for a bare task containing a multi-byte character at the previous byte cutoff boundary (verification: unit - add/update `src/openspec_cmd.rs` tests, then run `cargo test openspec_cmd`)

3. [x] Preserve the existing validation warning behavior for long bare tasks after the truncation change (verification: unit - `cargo test openspec_cmd` passes with assertions for `Possible task without checkbox` output)

4. [x] Update validation spec coverage for UTF-8-safe bare-task previews (verification: unit - `openspec/changes/fix-utf8-task-preview-slicing/specs/cflx-proposal-validation/spec.md` captures the behavior and `cflx openspec validate fix-utf8-task-preview-slicing --strict` passes)
