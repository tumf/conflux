## MODIFIED Requirements

### Requirement: Apply system prompt MUST include task format guidance

apply プロンプトは tasks.md のフォーマット修正と進捗更新の指示を含めなければならない（MUST）。active task section のトップレベル `- ` または `* ` 項目は checkbox task として記載しなければならず（MUST）、説明・検証結果・evidence を unchecked bullet として追加してはならない（MUST NOT）。Future Work / Out of Scope / Notes / Final Validation / Acceptance Notes / Implementation Blocker セクションは narrative non-task section として扱い、チェックボックスを含めてはならない（MUST NOT）。runtime-owned acceptance follow-up の finding evidence は正確に `  evidence: <one-line evidence>` と記載し、`- evidence:` を使用してはならない（MUST NOT）。WIP スナップショット作成を妨げないため、apply プロンプトは `--no-verify` を一律禁止してはならない（MUST NOT）。

#### Scenario: apply プロンプトが `--no-verify` を一律禁止しない

- **GIVEN** apply プロンプトを生成する
- **WHEN** 進捗スナップショットの作成を行う
- **THEN** プロンプトに `--no-verify` の一律禁止が含まれない

#### Scenario: Future Work へ移動したタスクのチェックボックスを除去する

- **GIVEN** tasks.md に人間作業のタスクがある
- **WHEN** エージェントがタスクを narrative non-task section へ移動する
- **THEN** タスクはチェックボックスなしの prose または metadata として記載される
- **AND** task_parser はその内容を進捗計算に含めない

#### Scenario: active section に evidence bullet を追加しない

**Given**: apply agent records implementation evidence in `tasks.md`
**When**: the evidence is outside the runtime-owned acceptance follow-up
**Then**: guidance prohibits a top-level `- evidence:` or equivalent unchecked bullet in an active task section
**And**: guidance directs longer narrative evidence to a narrative non-task section

#### Scenario: acceptance finding evidence uses canonical indentation

**Given**: apply completes a repository finding in the runtime-owned acceptance follow-up
**When**: it records one-line evidence
**Then**: guidance requires the exact indented `  evidence: <one-line evidence>` form
**And**: guidance prohibits `- evidence:` and unindented evidence labels
