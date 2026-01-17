## MODIFIED Requirements
### Requirement: Apply system prompt MUST include task format guidance

apply プロンプトは tasks.md のフォーマット修正と進捗更新の指示を含めなければならない（MUST）。加えて、WIP スナップショット作成を妨げないため、apply プロンプトは `--no-verify` を一律禁止してはならない（MUST NOT）。

#### Scenario: apply プロンプトが `--no-verify` を一律禁止しない
- **GIVEN** apply プロンプトを生成する
- **WHEN** 進捗スナップショットの作成を行う
- **THEN** プロンプトに `--no-verify` の一律禁止が含まれない
