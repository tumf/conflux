## 1. WebStateをTUIに渡すための準備
- [x] 1.1 `src/tui/runner.rs`に`WebStateRef`型エイリアスを追加し、`run_tui`関数シグネチャに`WebStateRef`パラメータを追加
- [x] 1.2 `run_tui_loop`関数に`WebStateRef`パラメータを追加
- [x] 1.3 web-monitoringフィーチャーの有無で型を切り替え（`Option<Arc<WebState>>` or `Option<()>`）

## 2. TUIオーケストレーターへのWebState受け渡し
- [x] 2.1 `src/tui/orchestrator.rs`の`run_orchestrator`関数に`WebStateRef`パラメータを追加
- [x] 2.2 `src/tui/orchestrator.rs`の`run_orchestrator_parallel`関数に`WebStateRef`パラメータを追加
- [x] 2.3 `run_tui_loop`からオーケストレーター関数呼び出し時に`web_state`を渡す

## 3. 状態更新のブロードキャスト実装
- [x] 3.1 `run_orchestrator`のPhase 2で`list_changes_native()`呼び出し後に`web_state.update()`を追加
- [x] 3.2 `run_orchestrator`でapply完了後に`web_state.update()`を追加
- [x] 3.3 `run_orchestrator_parallel`でバッチ処理完了後に`web_state.update()`を追加

## 4. main.rsでのWebState受け渡し修正
- [x] 4.1 TUIモード（`Commands::Tui`）で`web_state`を`run_tui`に渡すように修正
- [x] 4.2 デフォルトTUIモード（`None`ケース）でも`web_state`をサポート（現状はNone）

## 5. テストと検証
- [x] 5.1 `cargo build --features web-monitoring`でビルドが通ることを確認
- [x] 5.2 `cargo build`（フィーチャーなし）でもビルドが通ることを確認
