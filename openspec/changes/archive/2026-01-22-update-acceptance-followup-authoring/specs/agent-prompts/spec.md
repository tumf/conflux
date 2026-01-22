## ADDED Requirements
### Requirement: Acceptance prompt MUST instruct tasks.md follow-up updates on FAIL
acceptance プロンプトは、FAIL を出力する場合に `openspec/changes/{change_id}/tasks.md` を直接更新する手順を明記しなければならない（MUST）。
指示には、`## Acceptance #<n> Failure Follow-up` セクションの追加（または既存セクションの更新）、`- [ ] <finding>` の 1 行 1 finding 形式、`ACCEPTANCE:`/`FINDINGS:` 行を tasks.md に追加しないことを含めなければならない（MUST）。
`<n>` は tasks.md 内の既存の `Acceptance #<n> Failure Follow-up` を基準に決定するよう指示しなければならない（MUST）。

#### Scenario: Acceptance prompt guides follow-up authoring
- **GIVEN** acceptance プロンプトが生成される
- **WHEN** エージェントが FAIL を出力する必要がある
- **THEN** プロンプトに tasks.md の follow-up 追記手順が含まれる
- **AND** `ACCEPTANCE:` や `FINDINGS:` を tasks.md に追加しない指示が含まれる
