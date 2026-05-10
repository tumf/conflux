## Implementation Tasks

- [ ] Reclassify successful hook stderr as informational observability output. (verification: unit - update/add `src/hooks.rs` tests so a zero-exit hook that writes stderr emits non-warning output while preserving the stderr text)
- [ ] Preserve failure stderr as warning/error context. (verification: unit - update/add `src/hooks.rs` tests so a non-zero hook that writes stderr still emits warning/error-visible output before `HookFailed` is returned)
- [ ] Preserve `on_merged` failure blocking semantics. (verification: unit - run/extend existing `src/tui/state/event_handlers/errors.rs` and `src/hooks.rs` tests covering `on_merged` hook failure and merge-wait display)
- [ ] Preserve output truncation behavior. (verification: unit - run existing `truncate_hook_output` and long hook output tests in `src/hooks.rs`)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate classify-hook-stderr-severity --archive-gate`
