## Implementation Tasks

- [x] Task 1: `MergeDeferred` の待機理由を reducer と parallel 実行側で区別できる状態モデルを定義する (verification: `src/orchestration/state.rs`, `src/parallel/queue_state.rs`, `src/events.rs` に自動再開対象と手動待機対象の遷移根拠が追加される)
- [x] Task 2: 先行 merge / resolve 完了後に deferred change を自動再評価し、`MergeWait` に残さず `ResolveWait` / `Resolving` / merge 再試行へ進める scheduler フローを実装する (verification: `src/parallel/queue_state.rs`, `src/parallel/dispatch.rs`, `src/parallel_run_service.rs` に再評価トリガーと遷移処理が追加される)
- [x] Task 3: TUI と shared reducer の表示同期を更新し、自動再開対象の待機が `MergeWait` に見え続けないようにする (verification: `src/tui/state.rs`, `src/tui/runner.rs`, `src/orchestration/state.rs` の表示/イベント同期が新しい待機意味論に一致する)
- [x] Task 4: 先行 merge 完了後に後続 change が stuck しない回帰テストを追加する (verification: `cargo test` で `src/orchestration/state.rs` / `src/tui/state.rs` / `src/parallel/tests` の該当テストが通る)

## Future Work

- 実運用で `MergeDeferred` 理由分類のログ粒度が不足する場合は observability 改善を別提案で行う
