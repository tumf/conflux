# タスク一覧：apply プロンプトで実行可能タスクへ正規化

## 1. 仕様（OpenSpec）

- [x] `openspec/specs/configuration/spec.md` の該当要件を確認し、delta を `openspec/changes/update-apply-prompt-actionability/specs/configuration/spec.md` に追加する
- [x] `openspec validate update-apply-prompt-actionability --strict` を実行し、指摘を解消する

## 2. 実装（Rust）

- [x] `src/agent.rs` の `APPLY_SYSTEM_PROMPT` に「実行可能タスク正規化」ルールを追加する
  - 未完了チェックボックスは必ず実行可能であること
  - 実行不能タスクは、まず具体コマンド+合格基準のタスクへ書き換えること
  - 人間判断が必須な場合のみ `(future work)` としてチェックボックスから外すこと
  - apply 成功（exit 0）なのに未完了が残る状態を許容しないこと

- [x] `src/agent.rs` の既存テストを更新し、プロンプトの必須文言が含まれることを検証する
- [x] 必要に応じてテンプレート/設定例（`.openspec-orchestrator.jsonc.example` 等）を更新し、挙動を説明する

## 3. 検証

- [x] `cargo test` を実行して成功する
- [x] `cargo fmt --check` を実行して差分がない
- [x] `cargo clippy -- -D warnings` を実行して警告がない

## 4. 再現防止（動作確認）

- [x] 抽象的な未完了タスクを含む change を1つ用意し、apply が「タスクの具体化→完了」まで進むことを確認する
  - 例: 「〜を確認」「容易になった」などのタスクが、コマンド+合格基準に置換される
- [x] apply 成功（exit 0）後に抽象タスクが未完了のまま残らないことを確認する
