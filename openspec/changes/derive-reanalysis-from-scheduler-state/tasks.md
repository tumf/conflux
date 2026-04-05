## Implementation Tasks

- [x] `src/parallel/orchestration.rs` のメインループ先頭に状態駆動の reanalysis 判定を追加する: `!queued.is_empty() && available_slots > 0 && debounce_elapsed()` (verification: `cargo test --lib parallel`)
- [x] `src/parallel/mod.rs` の `needs_reanalysis: bool` フィールドを削除するか、状態導出キャッシュに変更する (verification: `cargo test --lib parallel`)
- [x] `src/parallel/queue_state.rs` の `perform_reanalysis_and_dispatch()` 末尾の無条件 `self.needs_reanalysis = false` を削除する (verification: `cargo test --lib parallel`)
- [x] `src/parallel/orchestration.rs` の QueueNotification select 分岐から「needs_reanalysis セット漏れ」問題を構造的に解消する (verification: `cargo test --lib parallel`)
- [x] 各イベント分岐 (Completion, ResolveCompletion, QueueNotification 等) での `needs_reanalysis = true` 設定を削除またはログ専用に変更する (verification: `cargo test --lib parallel`)
- [x] テスト追加: resolving 1件 + slot 空き + queued 1件 → debounce 経過後に dispatch される (verification: `cargo test --lib parallel`)
- [x] テスト追加: dispatch 0件だった場合でも次ループで再評価される (verification: `cargo test --lib parallel`)

## Future Work

- ReanalysisReason をログ専用に整理し、制御フローから分離する
