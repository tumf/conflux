## MODIFIED Requirements
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

## ADDED Requirements
### Requirement: Acceptance MUST fail if excluded sections contain checkboxes
acceptance プロンプトは、Future Work / Out of Scope / Notes セクション内にチェックボックス（`- [ ]` または `- [x]`）が残っている場合、FAIL を出力し apply フェーズに戻さなければならない（MUST）。

#### Scenario: Future Work にチェックボックスが残っていたら FAIL
- **GIVEN** tasks.md の Future Work セクションに `- [ ] タスク` または `- [x] タスク` が存在する
- **WHEN** acceptance フェーズが実行される
- **THEN** acceptance は FAIL を出力する
- **AND** FINDINGS に「Future Work セクションにチェックボックスが残っている」旨を記載する
- **AND** apply フェーズに戻り、チェックボックスの削除が行われる
