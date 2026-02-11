## 1. Implementation
- [x] 1.1 acceptance の埋め込みプロンプト定数を空にし、テンプレート単一ソース前提のコメントへ更新する
  - 検証: src/config/defaults.rs に埋め込み本文が存在しないことを確認する
- [x] 1.2 acceptance プロンプト生成を context_only 相当に統一し、固定手順を注入しない
  - 検証: src/agent/prompt.rs の build_acceptance_prompt が context_only と同等の構築を行うことを確認する
- [x] 1.3 acceptance_prompt_mode の full を互換エイリアスとして扱い、実際の生成結果が変わらないことを明記する
  - 検証: src/config/mod.rs の AcceptancePromptMode と関連コメントを確認する
- [x] 1.4 既存テストを新挙動に合わせて更新する
  - 検証: src/agent/prompt.rs と src/config/defaults.rs の該当テストが新要件に一致する
- [x] 1.5 回帰確認として acceptance 実行コマンドに固定手順が二重挿入されないことを確認する
  - 検証: acceptance コマンド組み立てのログが change_id / path / diff などの可変コンテキストのみを含むことを確認する（例: src/agent/runner.rs か src/parallel/executor.rs）

## Acceptance #1 Failure Follow-up
- [x] `git status --porcelain` が空になるまでワーキングツリーをクリーンにする（未コミット変更: `openspec/changes/remove-acceptance-embedded-prompt/tasks.md`, `src/agent/prompt.rs`, `src/config/defaults.rs`）。
