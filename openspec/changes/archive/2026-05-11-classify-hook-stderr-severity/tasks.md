## Implementation Tasks

- [x] Reclassify successful hook stderr as informational observability output. (verification: unit - update/add `src/hooks.rs` tests so a zero-exit hook that writes stderr emits non-warning output while preserving the stderr text)
- [x] Preserve failure stderr as warning/error context. (verification: unit - update/add `src/hooks.rs` tests so a non-zero hook that writes stderr still emits warning/error-visible output before `HookFailed` is returned)
- [x] Preserve `on_merged` failure blocking semantics. (verification: unit - run/extend existing `src/tui/state/event_handlers/errors.rs` and `src/hooks.rs` tests covering `on_merged` hook failure and merge-wait display)
- [x] Preserve output truncation behavior. (verification: unit - run existing `truncate_hook_output` and long hook output tests in `src/hooks.rs`)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate classify-hook-stderr-severity --archive-gate`

## Acceptance #1 Failure Follow-up
- [x] Ensure failing hook execution errors are emitted through configured observability sinks after captured stderr warning output. (verification: unit - `cargo test hooks::tests::` passed in agent-exec job `de6d8d42da7e59051011234c3661aedf`; this includes `hooks::tests::test_hook_failure_stderr_event_log_remains_warn_before_error`, which verifies the stderr Warn entry precedes a hook failure Error entry)
