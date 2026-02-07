## 1. Implementation
- [x] 1.1 acceptance 判定語彙に `BLOCKED` を追加し、パーサーと列挙型を拡張する（確認: `src/acceptance.rs` に `test_parse_blocked` を追加し PASS/FAIL/CONTINUE と同様に通る）
- [x] 1.2 acceptance 実行結果に Blocked を追加し、serial/parallel の分岐を更新する（確認: `src/serial_run_service.rs` と `src/parallel/mod.rs` に Blocked ハンドリングが追加されている）
- [x] 1.3 apply プロンプトに Implementation Blocker の記録形式とエスカレーション出力手順を追加する（確認: `.opencode/commands/cflx-apply.md` に新しい手順がある）
- [x] 1.4 acceptance プロンプトに Implementation Blocker 審査と `ACCEPTANCE: BLOCKED` 出力条件を追加する（確認: `.opencode/commands/cflx-accept.md` に新しい手順がある）
- [x] 1.5 `BLOCKED` 判定時に apply ループを停止し、ワークスペースを保持する（確認: serial は change を停止扱いにし、parallel は workspace preserve ログを出す）
- [x] 1.6 Blocker 判定と停止動作のテストを追加する（確認: 関連ユニットテストが追加され `cargo test` で通る）

## 2. Validation
- [x] 2.1 `cargo test --lib` を実行し、全 855 テストが成功することを確認する

## Acceptance #1 Failure Follow-up
- [x] `ACCEPTANCE: BLOCKED` が serial CLI フローで終端状態になっていません。`src/serial_run_service.rs` の `process_acceptance_result` は `AcceptanceBlocked` を返すだけで、`src/orchestrator.rs` の `handle_change_result`/`handle_acceptance_result` は停止状態へ遷移させずに継続するため、後続ループで当該 change が再選択され archive に進む可能性があります。Blocked change を stalled/terminal として記録し、同一 change の apply 再試行と archive 進行を止めてください。
- [x] `ACCEPTANCE: BLOCKED` が TUI serial フローでも終端状態になっていません。`src/tui/orchestrator.rs` の `ChangeProcessResult::AcceptanceBlocked` 分岐はログ出力のみで `pending_changes` から除外せず、同ファイルの選択ロジックは completed change を archive 対象に含めるため、Blocked change が次周回で archive され得ます。TUI 側でも blocked change を停止扱いにして pending から外し、手動フォロー用に保持してください。
- [x] 上記 BLOCKED 停止動作の回帰テストを追加してください（少なくとも serial CLI と TUI で、`ACCEPTANCE: BLOCKED` 後に同一 change が再適用・archive されないことを検証）。
