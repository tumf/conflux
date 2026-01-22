## 1. 仕様更新
- [ ] 1.1 `openspec/changes/add-acceptance-git-clean-check/specs/agent-prompts/spec.md` に acceptance の git clean check 要件とシナリオを追加する（確認: spec.md に要件と Scenario が記載されている）

## 2. 実装
- [ ] 2.1 `src/config/defaults.rs` の `ACCEPTANCE_SYSTEM_PROMPT` に git status のクリーン確認（未追跡も含む）を追記する（確認: `git status --porcelain` が空であることを求める記述がある）
- [ ] 2.2 FAIL 時の FINDINGS に未コミット変更と未追跡ファイルを列挙する指示を追加する（確認: `ACCEPTANCE_SYSTEM_PROMPT` の FAIL 指示に追加されている）

## 3. 検証
- [ ] 3.1 acceptance プロンプト生成が system prompt を含むことを確認する（確認: `src/agent/prompt.rs` の `build_acceptance_prompt` が `ACCEPTANCE_SYSTEM_PROMPT` を先頭に含めていることを再確認する）
