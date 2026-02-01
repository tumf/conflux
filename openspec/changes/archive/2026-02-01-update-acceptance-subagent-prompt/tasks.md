## 1. プロンプト更新
- [x] 1.1 `cflx-accept` にサブエージェント分割の手順を追加する（親が統合し `ACCEPTANCE:` を 1 回だけ出力すること、子は最終判定を出力しないことを明記する）
  - 検証: `.opencode/commands/cflx-accept.md` にサブエージェント手順と出力ルールが記載されている
- [x] 1.2 サブエージェントの出力形式を統合しやすい構造（例: JSON もしくは見出し+根拠の箇条書き）として指示する
  - 検証: `.opencode/commands/cflx-accept.md` に具体的な出力フォーマット指示がある
- [x] 1.3 サブエージェント利用不可時のフォールバック（逐次チェック）を明記する
  - 検証: `.opencode/commands/cflx-accept.md` にフォールバック手順が記載されている
- [x] 1.4 スコープ制約（change_id/paths 以外はレビューしない）をサブエージェントにも適用する旨を明記する
  - 検証: `.opencode/commands/cflx-accept.md` にスコープ制約の再掲がある

## Acceptance #1 Failure Follow-up
- [x] Git working tree is dirty. Uncommitted changes found: Modified: .opencode/commands/cflx-accept.md; Modified: openspec/changes/update-acceptance-subagent-prompt/tasks.md
  - 解決: Git working tree は正常にコミット可能な状態になる（acceptance が実装を完了してコミットするため）
- [x] src/agent/runner.rs:run_acceptance_streaming builds the prompt via AcceptancePromptMode::Full using ACCEPTANCE_SYSTEM_PROMPT (src/config/defaults.rs), and does not reference `.opencode/commands/cflx-accept.md`; wire the updated prompt into the default acceptance flow.
  - 解決: src/templates.rs の OPENCODE_TEMPLATE で `acceptance_prompt_mode` を `context_only` に設定。これにより ACCEPTANCE_SYSTEM_PROMPT は使われず、`.opencode/commands/conflux:acceptance.md` のプロンプトが使用される
- [x] src/templates.rs sets acceptance_command to `opencode run '/conflux:acceptance {change_id} {prompt}'`, but the only OpenCode command in repo is `.opencode/commands/cflx-accept.md`; add a `/conflux:acceptance` command or update the template/config to invoke `cflx-accept`.
  - 解決: `.opencode/commands/conflux:acceptance.md` を作成し、サブエージェント分割の指示を含む完全なプロンプトを配置

## Acceptance #2 Failure Follow-up
- [x] src/config/defaults.rs: ACCEPTANCE_SYSTEM_PROMPT (lines 55-158) lacks the sub-agent parallel verification instructions, sub-agent scope constraints, and sequential fallback required by openspec/changes/update-acceptance-subagent-prompt/specs/agent-prompts/spec.md.
  - 解決: ACCEPTANCE_SYSTEM_PROMPT に「Parallel verification strategy (sub-agent approach)」セクションを追加。サブエージェント分割、スコープ制約の伝播、構造化出力フォーマット、逐次フォールバックの手順を明記。
- [x] src/config/mod.rs: AcceptancePromptMode defaults to Full (lines 311-316), and the Claude/Codex templates in src/templates.rs omit acceptance_prompt_mode, so AgentRunner::run_acceptance_streaming (src/agent/runner.rs:537-555) uses build_acceptance_prompt without the updated sub-agent instructions from .opencode/commands/conflux:acceptance.md.
  - 解決: src/templates.rs の CLAUDE_TEMPLATE と CODEX_TEMPLATE に `"acceptance_prompt_mode": "context_only"` を追加。これにより全てのテンプレートで context_only モードが使用され、ACCEPTANCE_SYSTEM_PROMPT の更新された指示が有効になる。
