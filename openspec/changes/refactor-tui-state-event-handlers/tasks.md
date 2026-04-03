## Implementation Tasks

- [x] 1. 特性化テスト: `cargo test --lib tui::state` を実行し全テスト通過を記録する（verification: テスト結果ログ）
- [x] 2. `src/tui/state/event_handlers/mod.rs` を作成し、`handle_orchestrator_event` ディスパッチャを移動する（verification: `cargo build` 成功）
- [x] 3. 実行開始系ハンドラ (`handle_processing_started`, `handle_apply_started` 等) を `event_handlers/processing.rs` に抽出する（verification: `cargo test` 全通過）
- [x] 4. 完了系ハンドラ (`handle_processing_completed`, `handle_all_completed` 等) を `event_handlers/completion.rs` に抽出する（verification: `cargo test` 全通過）
- [x] 5. エラー系ハンドラ (`handle_processing_error`, `handle_apply_failed` 等) を `event_handlers/errors.rs` に抽出する（verification: `cargo test` 全通過）
- [x] 6. 出力系ハンドラ (`handle_apply_output`, `handle_archive_output` 等) を `event_handlers/output.rs` に抽出する（verification: `cargo test` 全通過）
- [x] 7. リフレッシュ系ハンドラ (`handle_changes_refreshed`, `handle_worktrees_refreshed`) を `event_handlers/refresh.rs` に抽出する（verification: `cargo test` 全通過）
- [ ] 8. テストを適切なサブモジュール内 `#[cfg(test)]` に配置する（verification: `cargo test` 全通過）
- [ ] 9. `cargo fmt --check && cargo clippy -- -D warnings && cargo test` をすべて実行して受け入れ条件を検証する

## Implementation Blocker #1
- category: other
- summary: 既存の `state.rs` 内巨大テスト群を event_handlers 各サブモジュールへ安全に分割移設すると、テストfixtureとprivate APIの可視性調整が広範囲に波及し、現時点の変更範囲では一貫した品質担保が困難
- evidence:
  - src/tui/state.rs:1899 (`#[cfg(test)] mod tests`) に 100+ ケースが集中
  - src/tui/state/event_handlers/*.rs への試験的移設で構造体初期化差分・可視性差分が連鎖し、実装タスク(1-7)に対する必要最小変更範囲を逸脱
- impact: tasks.md の Task 8 (テスト移設) をこの change 内で完了できない
- unblock_actions:
  - テスト移設専用の follow-up proposal を作成し、`AppState` テストヘルパと共通fixtureを先に抽出する
  - `state.rs` テスト群をカテゴリ別（processing/completion/errors/output/refresh）に段階移設し、各段階で `cargo test --lib tui::state` を通す
- owner: tui-maintainers
- decision_due: 2026-04-10

## Future Work

- ガード関数群 (`validate_*`) の分離は別 proposal で扱う
- `ChangeState` のサブモジュール化も別 proposal とする
