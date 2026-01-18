## ADDED Requirements
### Requirement: changes間のspec delta衝突検出コマンド
CLI SHALL provide a subcommand to detect conflicts between spec delta files across changes without using an LLM.

#### Scenario: 衝突なしの場合の成功
- **WHEN** user runs the new conflict detection command
- **AND** no conflicting spec deltas are found
- **THEN** the command exits with status code 0

#### Scenario: 衝突が検出された場合
- **WHEN** user runs the new conflict detection command
- **AND** conflicting spec deltas are found
- **THEN** the command outputs conflict details
- **AND** the command exits with a non-zero status code

#### Scenario: JSON出力の指定
- **WHEN** user runs the new conflict detection command with a JSON output flag
- **THEN** the command outputs a machine-readable JSON payload
