## 1. 事前検証（3重送信パターンの特定）
- [ ] 1.1 `src/tui/orchestrator.rs` 内の `apply_execution_event` 呼び出しを全て列挙する（`rg "apply_execution_event" src/tui/orchestrator.rs`）。各呼び出し箇所で shared_state + web_state の2箇所に送信しているパターンを記録する。現時点で L265, L282, L285, L411, L414, L467, L491, L512, L515, L527 等が該当。
- [ ] 1.2 `src/orchestrator.rs` 内の同様のパターンを列挙する。
- [ ] 1.3 Web `apply_execution_event()` 内のステータス書き換え match arm（`src/web/state.rs` L462, L476, L485, L509, L515, L526, L533, L572, L582, L589, L599, L613, L684, L690）を全て記録する。

## 2. ヘルパー関数の導入
- [ ] 2.1 `src/tui/orchestrator.rs` に `async fn dispatch_event(shared_state, web_state, event)` ヘルパーを追加する。内部で `shared_state.write().await.apply_execution_event(&event)` を呼び、`#[cfg(feature = "web-monitoring")]` ガード下で `web_state.apply_execution_event(&event).await` を呼ぶ。
- [ ] 2.2 `src/tui/orchestrator.rs` 内の全ての「shared_state + web_state に個別送信」パターンを `dispatch_event()` 呼び出しに置き換える。
- [ ] 2.3 `src/orchestrator.rs` に同様のヘルパーを追加し、重複呼び出しを統一する。
- [ ] 2.4 `cargo test` が通ることを確認する。

## 3. Web apply_execution_event のステータス書き換え削除（refactor-unify-change-status 完了後）
- [ ] 3.1 `src/web/state.rs` の `apply_execution_event()` メソッド内の `change.queue_status = Some("...".to_string())` 全箇所を削除する（1.3 で列挙した行）。ログ・progress 更新など非ステータス処理は残す。
- [ ] 3.2 `ChangeStatus` の `queue_status` が Reducer の `display_status()` から `from_changes_with_shared_state()` 内で導出されていることを確認する（`src/web/state.rs` L144-148）。
- [ ] 3.3 `cargo test` が通ることを確認する。

## 4. TUI handle_event のステータス書き換え削除（refactor-unify-change-status 完了後）
- [ ] 4.1 `src/tui/state.rs` の各イベントハンドラ内のステータス書き換え箇所が `display_status_cache` 更新に変更されていることを確認する（refactor-unify-change-status の成果）。
- [ ] 4.2 ステータス更新が Reducer の `apply_execution_event()` で行われた後の `apply_display_statuses_from_reducer()` 呼び出しで TUI に反映されるフローになっていることを確認する。
- [ ] 4.3 `cargo test` と `cargo clippy -- -D warnings` が通ることを確認する。
