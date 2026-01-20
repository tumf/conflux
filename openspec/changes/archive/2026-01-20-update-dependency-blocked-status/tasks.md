## 1. Implementation
- [x] 1.1 依存待ちを示す queue status と表示語彙を追加する（確認: src/tui/types.rs と Web のステータス表示）
- [x] 1.2 依存関係でブロックされた change を状態更新イベントで通知する（確認: src/parallel/mod.rs と TUI/Web の state 更新）
- [x] 1.3 依存関係が解決されたら blocked を解除して queued に戻す（確認: analysis ループと queued 状態更新）
- [x] 1.4 TUI の描画と key hints を blocked 非アクティブ扱いに合わせる（確認: 表示とアクティブ判定）
- [x] 1.5 Web ダッシュボードのステータス語彙に blocked を追加する（確認: change 行の表示）
- [x] 1.6 blocked 状態の遷移テストを追加する（確認: cargo test もしくは該当テスト）

## 2. Validation
- [x] 2.1 cargo test を実行しステータス処理の挙動を確認する


## 3. Acceptance Failure Follow-up
- [x] 3.1 MergeWait 依存の skip reason 除外 - `skip_reason_for_change()` から `should_skip_due_to_merge_wait()` チェックを削除し、failed dependencies のみをスキップ理由とする（確認: MergeWait 依存は blocked/queued 状態として扱われる）
- [x] 3.2 Web イベント処理追加 - `WebState::apply_execution_event()` に `DependencyBlocked` と `DependencyResolved` イベント分岐を追加して `queue_status` を更新する（確認: Web で blocked/queued 表示が正しく動作）
- [x] 3.3 テスト修正 - `test_skip_reason_for_merge_deferred_dependency` を修正して MergeWait 依存が skip reason として返されないことを確認、`test_get_version_string` のバージョン文字列フォーマット期待値を修正（確認: cargo test 成功）


## 4. Acceptance Failure Follow-up (Round 2)
- [x] 4.1 スキップログの文言修正 - `src/parallel/mod.rs` の `execute_with_order_based_reanalysis()` 内のスキップログを `"Skipping change-{} because dependency change-{} failed"` フォーマットに変更する（確認: ログ出力が仕様の文言に一致、cargo test 成功）
