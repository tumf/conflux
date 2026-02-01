## 1. テンプレートとコマンドの整合
- [ ] 1.1 `src/templates.rs` の各テンプレートで `acceptance_prompt_mode` を `context_only` に統一する
  - 検証: `src/templates.rs` に `"acceptance_prompt_mode": "context_only"` が Claude/OpenCode/Codex 全てに含まれる
- [ ] 1.2 `acceptance_command` が `cflx-accept` を呼ぶ構成になるようテンプレートを更新する
  - 検証: `src/templates.rs` の `acceptance_command` が `/cflx-accept {change_id} {prompt}` を参照する

## 2. 既存プロンプトの役割整理
- [ ] 2.1 `ACCEPTANCE_SYSTEM_PROMPT` の役割を「固定手順ではなく可変コンテキスト補助」に限定する（必要なら削除・簡素化）
  - 検証: `src/config/defaults.rs` の acceptance 固定手順が `.opencode/commands/cflx-accept.md` と重複しない

## 3. 回帰確認
- [ ] 3.1 受け入れプロンプトが可変コンテキストのみを渡すことを確認する
  - 検証: `src/agent/runner.rs` の `run_acceptance_streaming()` が `AcceptancePromptMode::ContextOnly` で `build_acceptance_prompt_context_only()` を使用している
