# agent-prompts Specification

## Purpose

This specification defines the behavior and constraints for AI agent system prompts, particularly the apply prompt (`APPLY_SYSTEM_PROMPT`), to ensure reliable and autonomous task execution.
## Requirements
### Requirement: Apply system prompt MUST include task format guidance

apply プロンプトは tasks.md のフォーマット修正と進捗更新の指示を含めなければならない（MUST）。加えて、WIP スナップショット作成を妨げないため、apply プロンプトは `--no-verify` を一律禁止してはならない（MUST NOT）。

#### Scenario: apply プロンプトが `--no-verify` を一律禁止しない
- **GIVEN** apply プロンプトを生成する
- **WHEN** 進捗スナップショットの作成を行う
- **THEN** プロンプトに `--no-verify` の一律禁止が含まれない

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
