## MODIFIED Requirements

### Requirement: Acceptance prompt MUST instruct tasks.md follow-up updates on FAIL

acceptance プロンプトは、FAIL を出力する場合に `openspec/changes/{change_id}/tasks.md` を直接更新する手順を明記しなければならない（MUST）。

follow-up 記録は、repo 実装差分が必要な remediation と、archive-readiness / commit-path / external unblock の blocker-only finding を区別しなければならない（MUST）。

- remediation finding は `## Acceptance #<n> Failure Follow-up` 配下の unchecked checkbox として記録する
- blocker-only finding は同 section 内で non-checkbox note として記録し、raw progress を増やす apply-driving task として扱ってはならない（MUST NOT）
- `ACCEPTANCE:` / `FINDINGS:` 行を tasks.md に追加してはならない（MUST NOT）

#### Scenario: acceptance records remediation and blocker notes separately
- **GIVEN** acceptance が change `alpha` に対して 2 つの finding を持つ
- **AND** 1 つは repo 修正が必要な remediation
- **AND** 1 つは archive commit path を阻害する blocker-only finding
- **WHEN** acceptance が tasks.md follow-up を更新する
- **THEN** remediation は unchecked checkbox として記録される
- **AND** blocker-only finding は non-checkbox note として記録される
- **AND** runtime は blocker-only note を implementation progress 未完了の唯一の根拠として扱わない
