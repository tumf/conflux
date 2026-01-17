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

The apply system prompt MUST explicitly prohibit moving tasks to Future Work based on difficulty, regression risk, or need for additional testing (MUST NOT).

#### Scenario: Prohibit moving tasks to Future Work except for pre-marked items

**Given:**
- tasks.md contains high-difficulty items

**When:**
- apply agent determines implementation approach

**Then:**
- Agent does NOT move tasks to Future Work based solely on difficulty
- Agent treats only tasks already marked with `(future work)` as Future Work
