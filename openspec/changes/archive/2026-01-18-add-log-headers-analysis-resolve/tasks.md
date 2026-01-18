## 1. 仕様・設計
- [x] 1.1 既存ログ表示仕様の確認と変更点の整理（確認先: openspec/specs/cli/spec.md）
- [x] 1.2 ログヘッダ追加の設計方針を決定し proposal.md に反映（確認先: openspec/changes/add-log-headers-analysis-resolve/proposal.md）

## 2. 実装
- [x] 2.1 解析ログヘッダ表示の追加（確認先: src/tui/render.rs のログ描画処理）
- [x] 2.2 ResolveOutput に change_id を含めるイベント拡張（確認先: src/events.rs, src/tui/state/events.rs）
- [x] 2.3 resolve ログ出力時に change_id を設定（確認先: src/parallel/conflict.rs）
- [x] 2.4 既存ログテストの更新（確認先: src/tui/state/events.rs の該当テスト）

## 3. 検証
- [x] 3.1 変更に影響するテストを実行する（コマンド: cargo test）
- [x] 3.2 解析/resolve ログにヘッダが表示されることを確認する（確認先: TUI ログパネル表示）


## 4. Acceptance Failure Follow-up
- [x] 4.1 Fix render.rs to display operation-only headers when change_id is absent (確認先: src/tui/render.rs:766-792)
- [x] 4.2 Add unit test for analysis log header rendering (確認先: src/tui/render.rs or new test module)
- [x] 4.3 Add unit test for resolve log header rendering (確認先: src/tui/render.rs or new test module)
- [x] 4.4 Run all tests to verify fix (コマンド: cargo test)
- [x] 4.5 Verify analysis log headers display correctly in TUI (確認先: manual TUI inspection or existing tests)
