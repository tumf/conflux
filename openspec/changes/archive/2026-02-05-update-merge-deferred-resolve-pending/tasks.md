## 1. 実装
- [x] 1.1 TUI の `MergeDeferred` 受信処理を更新し、resolve 実行中は `ResolveWait` へ遷移し resolve 待ち行列へ追加する
  - 完了条件: `src/tui/state.rs` の `handle_merge_deferred` が resolve 実行中に `ResolveWait` と待ち行列追加を行う
  - 検証方法: `cargo test merge_deferred` を実行し、該当テストがパスする
- [x] 1.2 resolve 非実行時は `MergeWait` のまま維持するガードを追加する
  - 完了条件: resolve 実行中でない場合に `MergeWait` を保持する分岐が明確に実装されている
  - 検証方法: `cargo test merge_deferred` を実行し、該当テストがパスする
- [x] 1.3 Web 状態更新に `MergeDeferred` の待ち状態判定を反映し、`resolve pending` を表示語彙に追加する
  - 完了条件: `src/web/state.rs` が `MergeDeferred` を処理し、resolve 実行中なら `resolve pending` を返す
  - 検証方法: `cargo test web_state` を実行し、更新したテストがパスする

## 2. テスト
- [x] 2.1 `MergeDeferred` が resolve 実行中に `ResolveWait` へ遷移することを示す TUI テストを追加する
  - 完了条件: `src/tui/state.rs` のテストに待ち行列追加と `ResolveWait` の検証が含まれる
  - 検証方法: `cargo test merge_deferred` を実行し、該当テストがパスする
- [x] 2.2 `MergeDeferred` が resolve 非実行時に `MergeWait` を維持する TUI テストを追加する
  - 完了条件: `src/tui/state.rs` のテストに `MergeWait` 維持の検証が含まれる
  - 検証方法: `cargo test merge_deferred` を実行し、該当テストがパスする

## 3. 検証
- [x] 3.1 変更後の待ち状態が TUI と Web で一致することを確認する
  - 完了条件: TUI と Web のステータス語彙に `resolve pending` が含まれ、表示が一致する
  - 検証方法: `cargo test` を実行し、該当テストがパスする

## Acceptance #1 Failure Follow-up
- [x] WebState の `apply_execution_event` に `ExecutionEvent::MergeDeferred` の分岐がなく、`resolve pending`/`merge wait` が設定されないため Web UI が要件の待ち状態を表示できない（`src/web/state.rs` の `apply_execution_event`）。
