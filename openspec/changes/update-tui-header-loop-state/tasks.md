## Implementation Tasks

- [x] 1. `src/tui/render.rs` の `render_header` ステータス判定を mode 優先へ変更する（verification: `render_header` の match で `AppMode::Running` が `Ready` を返さないことを確認）
- [x] 2. `src/tui/render.rs` で Running 表示を `Running` / `Running <count>` の2形態に整理する（verification: `AppMode::Running` + active_count 0/2 の描画テストを追加・更新）
- [x] 3. `src/tui/render.rs` で Stopping 表示を `Stopping` に統一する（verification: `AppMode::Stopping` 時のヘッダー描画テストで `[Stopping]` を検証）
- [x] 4. 既存のヘッダーテスト期待値を新仕様に合わせて更新する（verification: `cargo test test_select_mode_shows_running_when_resolving` など関連テストの期待値見直し）
- [x] 5. 変更後の回帰確認を実行する（verification: `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings`）

## Future Work

- 必要に応じて Web UI 側でも「loop実行状態優先」の表示規約を統一
