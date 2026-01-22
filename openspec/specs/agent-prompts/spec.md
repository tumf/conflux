# agent-prompts Specification

## Purpose

This specification defines the behavior and constraints for AI agent system prompts, particularly the apply prompt (`APPLY_SYSTEM_PROMPT`), to ensure reliable and autonomous task execution.
## Requirements
### Requirement: Apply system prompt MUST include task format guidance
apply プロンプトは tasks.md のフォーマット修正と進捗更新の指示を含めなければならない（MUST）。Future Work / Out of Scope / Notes セクションへタスクを移動する際は、チェックボックス（`- [ ]` または `- [x]`）を削除し、プレーンテキストまたはチェックボックスなしのリスト項目として記載しなければならない（MUST）。WIP スナップショット作成を妨げないため、apply プロンプトは `--no-verify` を一律禁止してはならない（MUST NOT）。

#### Scenario: apply プロンプトが `--no-verify` を一律禁止しない
- **GIVEN** apply プロンプトを生成する
- **WHEN** 進捗スナップショットの作成を行う
- **THEN** プロンプトに `--no-verify` の一律禁止が含まれない

#### Scenario: Future Work へ移動したタスクのチェックボックスを除去する
- **GIVEN** tasks.md に人間作業のタスクがある
- **WHEN** エージェントがタスクを Future Work / Out of Scope / Notes セクションへ移動する
- **THEN** タスクはチェックボックスなしで記載される（例: `2.2 手動確認タスク` または `- 2.2 手動確認タスク`）
- **AND** task_parser はそのタスクを進捗計算に含めない

### Requirement: Apply system prompt MUST enforce non-interactive iteration

The apply system prompt (`APPLY_SYSTEM_PROMPT`) MUST explicitly state that the agent cannot ask questions to the user and must continue working until MaxIteration is reached, making autonomous decisions under operational constraints.

#### Scenario: Continue iteration without asking questions

**Given:**
- apply execution encounters an uncertain decision point

**When:**
- apply agent processes tasks

**Then:**
- Agent does not ask questions to the user
- Agent makes best autonomous decision and proceeds
- Agent continues iteration until MaxIteration is reached

### Requirement: Future Work restrictions MUST be strictly enforced
Future Work への移動は、**人間の作業**、**外部システムのデプロイ/承認**、または**長時間待機が必要な検証**に限って許可されなければならない（MUST）。

面倒さ、難易度、テストの手間、回帰リスクなどを理由に Future Work へ移動してはならない（MUST NOT）。

#### Scenario: 人間作業や外部作業のみ Future Work へ移動する
- **GIVEN** tasks.md に人間作業や外部デプロイが必要なタスクがある
- **AND** tasks.md に難易度が高いが自動化可能なタスクがある
- **WHEN** apply エージェントがタスクの扱いを判断する
- **THEN** 人間作業や外部デプロイのタスクのみ Future Work に移動する
- **AND** 自動化可能なタスクは Future Work に移動しない

### Requirement: Acceptance MUST fail if excluded sections contain checkboxes
acceptance プロンプトは、Future Work / Out of Scope / Notes セクション内にチェックボックス（`- [ ]` または `- [x]`）が残っている場合、FAIL を出力し apply フェーズに戻さなければならない（MUST）。

#### Scenario: Future Work にチェックボックスが残っていたら FAIL
- **GIVEN** tasks.md の Future Work セクションに `- [ ] タスク` または `- [x] タスク` が存在する
- **WHEN** acceptance フェーズが実行される
- **THEN** acceptance は FAIL を出力する
- **AND** FINDINGS に「Future Work セクションにチェックボックスが残っている」旨を記載する
- **AND** apply フェーズに戻り、チェックボックスの削除が行われる

### Requirement: Acceptance prompt MUST instruct tasks.md follow-up updates on FAIL
acceptance プロンプトは、FAIL を出力する場合に `openspec/changes/{change_id}/tasks.md` を直接更新する手順を明記しなければならない（MUST）。
指示には、`## Acceptance #<n> Failure Follow-up` セクションの追加（または既存セクションの更新）、`- [ ] <finding>` の 1 行 1 finding 形式、`ACCEPTANCE:`/`FINDINGS:` 行を tasks.md に追加しないことを含めなければならない（MUST）。
`<n>` は tasks.md 内の既存の `Acceptance #<n> Failure Follow-up` を基準に決定するよう指示しなければならない（MUST）。

#### Scenario: Acceptance prompt guides follow-up authoring
- **GIVEN** acceptance プロンプトが生成される
- **WHEN** エージェントが FAIL を出力する必要がある
- **THEN** プロンプトに tasks.md の follow-up 追記手順が含まれる
- **AND** `ACCEPTANCE:` や `FINDINGS:` を tasks.md に追加しない指示が含まれる
