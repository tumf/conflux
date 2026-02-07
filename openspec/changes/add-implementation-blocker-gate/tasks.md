## 1. Implementation
- [x] 1.1 acceptance 判定語彙に `BLOCKED` を追加し、パーサーと列挙型を拡張する（確認: `src/acceptance.rs` に `test_parse_blocked` を追加し PASS/FAIL/CONTINUE と同様に通る）
- [x] 1.2 acceptance 実行結果に Blocked を追加し、serial/parallel の分岐を更新する（確認: `src/serial_run_service.rs` と `src/parallel/mod.rs` に Blocked ハンドリングが追加されている）
- [x] 1.3 apply プロンプトに Implementation Blocker の記録形式とエスカレーション出力手順を追加する（確認: `.opencode/commands/cflx-apply.md` に新しい手順がある）
- [x] 1.4 acceptance プロンプトに Implementation Blocker 審査と `ACCEPTANCE: BLOCKED` 出力条件を追加する（確認: `.opencode/commands/cflx-accept.md` に新しい手順がある）
- [x] 1.5 `BLOCKED` 判定時に apply ループを停止し、ワークスペースを保持する（確認: serial は change を停止扱いにし、parallel は workspace preserve ログを出す）
- [x] 1.6 Blocker 判定と停止動作のテストを追加する（確認: 関連ユニットテストが追加され `cargo test` で通る）

## 2. Validation
- [x] 2.1 `cargo test --lib` を実行し、全 855 テストが成功することを確認する
