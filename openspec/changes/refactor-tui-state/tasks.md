## 1. 準備

- [ ] 1.1 `src/tui/state/` ディレクトリを作成
- [ ] 1.2 基本的なモジュール構造を設定 (`mod.rs`)

## 2. ChangeState の分離

- [ ] 2.1 `ChangeState` 構造体を `src/tui/state/change.rs` に移動
- [ ] 2.2 関連メソッド（`from_change`, `progress_percent` など）を移動
- [ ] 2.3 `ChangeState` のテストを移動

## 3. モード関連の分離

- [ ] 3.1 `AppMode` enum を `src/tui/state/modes.rs` に移動
- [ ] 3.2 `start_processing`, `toggle_parallel_mode` を移動
- [ ] 3.3 モード関連テストを移動

## 4. ログ管理の分離

- [ ] 4.1 ログ関連定数 (`MAX_LOG_ENTRIES`) を `src/tui/state/logs.rs` に移動
- [ ] 4.2 `add_log`, `scroll_logs_*` メソッドを移動
- [ ] 4.3 ログ関連テストを移動

## 5. イベントハンドリングの分離

- [ ] 5.1 `handle_orchestrator_event` を `src/tui/state/events.rs` に移動
- [ ] 5.2 `update_changes` メソッドを移動
- [ ] 5.3 イベント関連テストを移動

## 6. メインモジュールの整理

- [ ] 6.1 残りの `AppState` メソッドを `src/tui/state/mod.rs` に配置
- [ ] 6.2 必要な型を re-export
- [ ] 6.3 `src/tui/state.rs` を削除

## 7. 依存関係の更新

- [ ] 7.1 `src/tui/mod.rs` のインポートを更新
- [ ] 7.2 `src/tui/render.rs` のインポートを更新
- [ ] 7.3 `src/tui/runner.rs` のインポートを更新

## 8. テストと検証

- [ ] 8.1 `cargo test` で全テスト通過を確認
- [ ] 8.2 `cargo clippy` で警告がないことを確認
- [ ] 8.3 TUI を実際に起動して動作確認
