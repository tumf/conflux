## 1. 実装
- [x] 1.1 Logsビューのログヘッダはchange_idを含める一方、変更一覧のログプレビューは短縮形式を維持する旨を仕様に反映する。検証: `openspec/changes/update-tui-log-view-headers/specs/tui-architecture/spec.md` の記述を確認する
- [x] 1.2 Logsビューのヘッダ生成を更新し、change_idがあるログは`[{change_id}:{operation}:{iteration}]`/`[{change_id}:{operation}]`で描画する。検証: `src/tui/render.rs` のLogsビュー描画でchange_idが含まれることを確認する
- [x] 1.3 変更一覧のログプレビューは短縮ヘッダのままであることを維持する。検証: `src/tui/render.rs` の変更一覧プレビュー描画が`[operation:iteration]`/`[operation]`であることを確認する
- [x] 1.4 ログヘッダ描画のテスト期待値を更新し、Logsビューでchange_idが表示されることを追加で検証する。検証: `cargo test` が成功する

## Acceptance #1 Failure Follow-up
- [x] src/tui/render.rs: `test_log_header_analysis_without_iteration` expects `[analysis]`, and `render_logs()` renders `[analysis]` when iteration is None, but the spec requires analysis logs to always include iteration and display `[analysis:{iteration}]`. Enforce iteration for analysis logs and update tests accordingly.
