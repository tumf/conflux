## Implementation Tasks

- [x] 1. Characterization: 現在の進捗解析と fallback 順序を固定するテストを追加する。verification: unit - `cargo test task_parser` で `parse_progress_with_fallback` の worktree active、worktree archive、base archive、base active の優先順位が既存通りであることを確認する。completion: 既存挙動を変えずに失敗時の探索順序・成功時の件数がテストで明示されている。

- [x] 2. Characterization: 非推奨 API の互換挙動を固定するテストを追加する。verification: unit - `cargo test task_parser` で `parse_change_with_worktree_fallback`、`parse_archived_change`、`parse_archived_change_with_worktree_fallback` の成功・not found 経路が既存と同等であることを確認する。completion: 非推奨 API を内部共通化しても呼び出し元の結果が変わらないことをテストで示している。

- [x] 3. Path resolution を内部ヘルパーへ抽出し、`parse_progress_with_fallback` と非推奨 API から利用する。verification: unit - `cargo test task_parser` が成功し、同一 fallback 順序の重複実装が整理されていることをコードレビューで確認する。completion: `src/task_parser.rs` の fallback 順序が単一の内部表現または helper 群で表現されている。

- [x] 4. acceptance follow-up 書き込みの既存挙動を保持したまま、進捗 path resolution から責務を分離する。verification: unit - `cargo test record_acceptance_follow_up` が成功し、既存 section の置換、空 findings の既定文言、末尾改行処理が維持されている。completion: follow-up 書き込みロジックが進捗ファイル探索の helper と混ざらず、テストで追跡可能になっている。

- [x] 5. 最終回帰確認を実行する。verification: integration - `cargo fmt --check` と `cargo test` が成功する。completion: 既定テストスイートが成功し、CLI/API の公開挙動変更がないことを確認している。

## Future Work

非推奨 API の削除可否は別提案で判断する。

## Final Validation

実装後の OpenSpec 最終確認は `cflx openspec validate refactor-task-progress-resolution --strict` を使用する。
