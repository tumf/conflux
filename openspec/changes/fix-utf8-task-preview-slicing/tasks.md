# Tasks

## Implementation

1. [x] Replace the bare-task warning preview slice in `src/openspec_cmd.rs` with a UTF-8-safe truncation path (verification: unit - `src/openspec_cmd.rs` no longer uses `&trimmed[..trimmed.len().min(50)]` for bare-task preview rendering)

2. [x] Add regression coverage for a bare task containing a multi-byte character at the previous byte cutoff boundary (verification: unit - add/update `src/openspec_cmd.rs` tests, then run `cargo test openspec_cmd`)

3. [x] Preserve the existing validation warning behavior for long bare tasks after the truncation change (verification: unit - `cargo test openspec_cmd` passes with assertions for `Possible task without checkbox` output)

4. [x] Update validation spec coverage for UTF-8-safe bare-task previews (verification: unit - `openspec/changes/fix-utf8-task-preview-slicing/specs/cflx-proposal-validation/spec.md` captures the behavior and `cflx openspec validate fix-utf8-task-preview-slicing --strict` passes)

## Acceptance #1 Failure Follow-up
- [ ] Archive commit readiness is blocked by the same commit-path cleanliness issue required by .opencode/commands/cflx-accept.md:54-56. The real archive commit path cannot succeed while the workspace still contains pending dashboard/dist artifact updates; reconcile dashboard/dist/index.html with dashboard/dist/assets/index-Bd3Kf0Z0.js and dashboard/dist/assets/index-Blko_xDv.css, and ensure no leftover generated-file drift remains before archive.
- [ ] git status --porcelain is not empty for the required clean-working-tree acceptance check: dashboard/dist/index.html references /dashboard/assets/index-Bd3Kf0Z0.js and /dashboard/assets/index-Blko_xDv.css (dashboard/dist/index.html:8-9), but the acceptance diff context and changed-files status still report old generated asset paths dashboard/dist/assets/index-2C2C4k9E.js and dashboard/dist/assets/index-CMIaf2tt.css as changed. Regenerate or re-stage dashboard/dist so index.html and the referenced asset filenames are consistent, then re-run git status --porcelain until it is empty.
