## 1. 実装
- [x] 1.1 解析コマンド実行（ストリーミング含む）を専用ヘルパーに分割する（検証: コマンド起動と出力収集がヘルパー経由になっていることを確認）
- [x] 1.2 出力解析・JSON抽出・検証を専用ヘルパーに分割する（検証: 解析ロジックが単一ヘルパーに集約されていることを確認）
- [x] 1.3 既存のプロンプト内容が変わらないことを確認する（検証: 生成されるプロンプト文字列の差分がないことを確認）
- [x] 1.4 既存の挙動維持を確認するため `cargo test` を実行する（検証: `cargo test` が成功）

## 2. Acceptance #1 Failure Follow-up
- [x] 2.1 src/analyzer.rs:279 (build_parallelization_prompt) を修正して、すべての変更を含め、`is_approved = true` の変更は `[x]`、`is_approved = false` の変更は `[ ]` でマークする
- [x] 2.2 すべての変更に proposal パス (`openspec/changes/{change_id}/proposal.md`) を含める
- [x] 2.3 既存テスト (`test_build_prompt_with_selected_markers`, `test_build_prompt_none_selected`) を新仕様に合わせて更新
- [x] 2.4 `cargo test` を実行して全テストが成功することを確認（検証: 1009 tests passed）

## 3. Acceptance #2 Failure Follow-up
- [x] 3.1 Git working tree をクリーンにする（検証: `git status --porcelain` が空であることを確認）
