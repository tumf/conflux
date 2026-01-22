## 1. 実装

- [x] 1.1 `QueueStatus::is_active` を更新し、Running ヘッダーのカウントが queued を含まないようにする。確認: `src/tui/types.rs` の `test_queue_status_is_active` を更新し、`cargo test test_queue_status_is_active` が通る。
- [x] 1.2 手動 resolve の開始/完了を並列スケジューラの in-flight 管理に反映し、スロットが空くまで queued が dispatch されないようにする。確認: `src/parallel/mod.rs` の in-flight 計算で resolve 中は `available_slots` が減ることをユニットテストで確認する。
- [x] 1.3 手動 resolve が in-flight に含まれることを検証するテストを `src/parallel/tests/` に追加する。確認: `cargo test parallel::tests::` の該当テストが通る。

## 2. 仕上げ

- [x] 2.1 Running モードのヘッダー表示が in-flight のみを数えることを確認するため、TUI 進行状態のテストを更新する。確認: 追加したテストで queued がカウントされないことを検証する。


## Acceptance #1 Failure Follow-up

- [x] 3.1 `ResolveMergesWithRetryArgs` に `auto_resolve_count` フィールドを追加する。確認: `src/parallel/conflict.rs` で構造体定義を更新。
- [x] 3.2 `resolve_merges_with_retry` の開始時に `auto_resolve_count` を increment し、終了時（成功/失敗）に decrement する。確認: `src/parallel/conflict.rs` の `resolve_merges_with_retry` 関数。
- [x] 3.3 `resolve_conflicts_with_retry` にも同様に `auto_resolve_count` 引数を追加し、開始/終了時にカウンタを更新する。確認: `src/parallel/conflict.rs` の `resolve_conflicts_with_retry` 関数。
- [x] 3.4 `ParallelExecutor::execute_with_order_based_reanalysis` の `available_slots` 計算で `manual_resolve_count` に加えて `auto_resolve_count` も減算する。確認: `src/parallel/mod.rs:566` 付近。
- [x] 3.5 `ParallelExecutor` から `resolve_merges_with_retry` と `resolve_conflicts_with_retry` の呼び出し箇所を更新し、新しい `auto_resolve_count` 引数を渡す。確認: `src/parallel/mod.rs` のすべての呼び出し箇所。
- [x] 3.6 自動 resolve のスロット消費を確認するテストを `src/parallel/tests/` に追加する。確認: `cargo test parallel::tests::` で該当テストが通る。

## Acceptance #2 Failure Follow-up

- [x] 4.1 `src/parallel/mod.rs:683` の `execute_with_order_based_reanalysis()` で依存解析後に `available_slots` を再計算する際、`auto_resolve_count` も減算する。確認: `cargo test parallel::tests::` で関連テストが通る。

## Acceptance #3 Failure Follow-up (Early Return Fix)

- [x] 5.1 `AutoResolveGuard` RAII ガード構造体を `src/parallel/conflict.rs` に追加し、コンストラクタで `auto_resolve_count` を increment、`Drop` 実装で decrement する。確認: `src/parallel/conflict.rs` に `AutoResolveGuard` 構造体と `Drop` 実装が存在する。
- [x] 5.2 `resolve_conflicts_with_retry()` の開始時に `AutoResolveGuard` を作成し、手動の increment/decrement 呼び出しをすべて削除する。確認: `src/parallel/conflict.rs` の `resolve_conflicts_with_retry()` 関数で `_guard = AutoResolveGuard::new(auto_resolve_count)` が呼ばれ、手動の `fetch_add`/`fetch_sub` 呼び出しが存在しない。
- [x] 5.3 `resolve_merges_with_retry()` の開始時に `AutoResolveGuard` を作成し、手動の increment/decrement 呼び出しをすべて削除する。確認: `src/parallel/conflict.rs` の `resolve_merges_with_retry()` 関数で `_guard = AutoResolveGuard::new(auto_resolve_count)` が呼ばれ、手動の `fetch_add`/`fetch_sub` 呼び出しが存在しない。
- [x] 5.4 すべてのテストが通ることを確認する。確認: `cargo test` で 899 個すべてのテストが成功する。
- [x] 5.5 Clippy 警告がないことを確認する。確認: `cargo clippy -- -D warnings` で警告なし。
- [x] 5.6 コードフォーマットが正しいことを確認する。確認: `cargo fmt --check` でフォーマットチェックOK。
