# タスク一覧: 外部依存ポリシーの統一（Mock-first）

## 実装タスク

- [x] `src/config/defaults.rs` の `ACCEPTANCE_SYSTEM_PROMPT` を更新し、外部依存を mock-first で扱う方針（非モック可能のみ Out of Scope、missing secret は FAIL）を明記（検証: プロンプト本文に該当方針が含まれる）
- [x] `~/.config/opencode/command/cflx-proposal.md` を更新し、proposal ステージでも外部依存を分類し mock-first でタスク化するガイドを追加（検証: 「外部依存の定義」「モック優先」「非モック可能の扱い」が明文化されている）
- [x] `~/.config/opencode/command/cflx-apply.md` を更新し、apply ステージでも外部依存を分類し mock-first で実装するガイドを追加（検証: missing secret を CONTINUE 理由にしない指示が含まれる）
- [x] 本 change の delta spec（`openspec/changes/update-acceptance-external-dependency-policy/specs/agent-prompts/spec.md`）と実装内容が整合するように調整（検証: 仕様に対して逸脱がない）

## テストタスク

- [x] `ACCEPTANCE_SYSTEM_PROMPT` に対して重要フレーズ（mock-first / missing secret => FAIL / out-of-scope 条件）が含まれることを検証する最小のユニットテストを追加（検証: `cargo test`）
- [x] 必要であれば `~/.config/opencode/command/*` の更新内容を静的に検証する最小テスト/チェックを追加（例: 期待フレーズの含有）（検証: `cargo test` または `cargo run -- ...` の実行ログ確認）

## 検証

- [x] `cargo fmt && cargo clippy -- -D warnings && cargo test`（検証: ローカルでエラーなく完走）
- [x] `npx @fission-ai/openspec@latest validate update-acceptance-external-dependency-policy --strict --no-interactive`（検証: OpenSpec の strict validation が通る）

## Acceptance #1 Failure Follow-up
- [x] `~/.config/opencode/command/openspec-apply.md` に外部依存ポリシー（mock-first）を追記し、`src/config/defaults.rs` の `DEFAULT_APPLY_COMMAND` が `/openspec-apply` を使う実運用フローでもポリシーが適用されるようにする
- [x] `~/.config/opencode/command/openspec-proposal.md` に外部依存ポリシー（mock-first）を追記し、proposal ステージの指示を仕様に一致させる
