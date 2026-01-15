# Tasks

## 実装

- [ ] 1. `src/agent.rs`の`APPLY_SYSTEM_PROMPT`にtasks.mdフォーマット修正ガイダンスを追加
  - チェックボックス必須の説明
  - 不正フォーマットのパターン（`## N.`, `- Task`, `1. Task`）
  - 修正方法（`- [ ]`への変換）
  - 0/0タスク検出時の対応手順

## 検証

- [ ] 2. 既存テストが全て通ることを確認
  - `cargo test`実行
  - エラーがないこと

- [ ] 3. 不正フォーマットのtasks.mdでapply実行テスト
  - テスト用changeを作成（不正フォーマットのtasks.md）
  - Sequential apply実行
  - AIが自動修正することを確認

- [ ] 4. コードフォーマットとlintチェック
  - `cargo fmt`実行
  - `cargo clippy`実行（警告なし）
