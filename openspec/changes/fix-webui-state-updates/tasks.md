## 1. WebStateをTUIに渡すための準備
- [ ] 1.1 `src/tui/mod.rs`の`run_tui`関数シグネチャに`Option<Arc<WebState>>`パラメータを追加
- [ ] 1.2 `src/tui/runner.rs`の`TuiRunner`構造体に`web_state`フィールドを追加
- [ ] 1.3 `TuiRunner::new`に`web_state`パラメータを追加

## 2. TUIオーケストレーターへのWebState受け渡し
- [ ] 2.1 `src/tui/orchestrator.rs`の`run_orchestrator`関数に`Option<Arc<WebState>>`パラメータを追加
- [ ] 2.2 `src/tui/orchestrator.rs`の`run_orchestrator_parallel`関数に`Option<Arc<WebState>>`パラメータを追加
- [ ] 2.3 `TuiRunner`からオーケストレーター関数呼び出し時に`web_state`を渡す

## 3. 状態更新のブロードキャスト実装
- [ ] 3.1 `run_orchestrator`のループ内で`openspec::list_changes_native()`呼び出し後に`web_state.update()`を追加
- [ ] 3.2 `run_orchestrator_parallel`のバッチ処理前後で`web_state.update()`を追加
- [ ] 3.3 アーカイブ完了後に状態更新をブロードキャスト

## 4. main.rsでのWebState受け渡し修正
- [ ] 4.1 TUIモード（`Commands::Tui`）で`web_state`を`run_tui`に渡すように修正
- [ ] 4.2 デフォルトTUIモード（`None`ケース）でも`web_state`をサポート（現状はNone）

## 5. テストと検証
- [ ] 5.1 `cargo build --features web-monitoring`でビルドが通ることを確認
- [ ] 5.2 TUIモードで`--web`オプション付きで起動し、ブラウザでダッシュボードが更新されることを確認
