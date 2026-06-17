## Implementation Tasks

- [x] 1. `is_blocked_only_scheduler_state` に resolve_wait/reject_wait の空チェックを追加する（`src/parallel/queue_state.rs:2275-2287`、`in_flight.is_empty()` の前に早期 `false` ガードを追加）(verification: unit - `cargo test -p conflux queue_state`; executed as actual package `cargo test -p cflx queue_state`, job 9fe479e55285aa0117f05910c89df461)

- [x] 2. resolve_wait 存在時に `is_blocked_only_scheduler_state` が `false` を返すテストを追加する（`src/parallel/tests/executor.rs`、executor の `resolve_wait_changes` に change を追加し dependency-blocked change が queued にある状態で `is_blocked_only_scheduler_state` が `false` を返すことを assert）(verification: integration - `cargo test test_blocked_only_resolve_wait_present`, job 07d58536a53c094ea785e3c7b340e10b)

- [x] 3. resolve_wait 完了後に dependency_blocked な change が dispatch されるエンドツーエンドテストを追加する（`src/parallel/tests/executor.rs`、resolve_wait change を設定→スケジューラが早期終了しない→resolve 完了→依存 change が dispatch されることを確認）(verification: integration - `cargo test test_resolve_wait_completion_unblocks_dependents`, job 22e371742e85e79442b9bdd756e4361c)

- [x] 4. 既存テストのリグレッション確認（`cargo test` 全テスト通過、特に `test_resolve_wait_does_not_block_queue_reanalysis_dispatch`、`test_resolving_with_free_slot_still_dispatches_queued_change`、`test_blocked_only_reanalysis_skips_analyzer` が影響を受けないこと）(verification: unit+integration - `cargo test`, job b90b53fb67befcde14110ce980f7f635)

## Final Validation

Archive validation 自体が authoritative な最終 OpenSpec validation gate です。
Expected archive gate: `cflx openspec validate fix-blocked-only-resolve-wait-skip --archive-gate`
