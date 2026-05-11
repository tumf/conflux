## Implementation Tasks

- [x] Reclassify successful hook stderr as informational observability output. (verification: unit - update/add `src/hooks.rs` tests so a zero-exit hook that writes stderr emits non-warning output while preserving the stderr text)
- [x] Preserve failure stderr as warning/error context. (verification: unit - update/add `src/hooks.rs` tests so a non-zero hook that writes stderr still emits warning/error-visible output before `HookFailed` is returned)
- [x] Preserve `on_merged` failure blocking semantics. (verification: unit - run/extend existing `src/tui/state/event_handlers/errors.rs` and `src/hooks.rs` tests covering `on_merged` hook failure and merge-wait display)
- [x] Preserve output truncation behavior. (verification: unit - run existing `truncate_hook_output` and long hook output tests in `src/hooks.rs`)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate classify-hook-stderr-severity --archive-gate`

## Acceptance #1 Failure Follow-up
- [x] `cargo test hooks::tests::` が失敗しています。失敗テストは `hooks::tests::test_hook_failure_stderr_event_log_remains_warn_before_error` で、`src/hooks.rs:1891` の期待「stderr warning の後に hook failure error が ExecutionEvent::Log に出る」を満たしていません。実際のイベントは `Running post_apply hook...` の Info と `post_apply hook stderr: failure diagnostic` の Warn の2件のみで、`post_apply hook failed` の Error ログイベントが送信されていません（agent-exec job `194e64978327d067e8d273a4e1297566`, stdout.log lines 79-89）。`src/hooks.rs` の失敗経路で `error!` だけでなく event_tx/output_handler へもエラーレベルの失敗ログを流すか、テスト/タスクの期待を実装実態に合わせて修正してください。
