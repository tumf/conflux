# Parallel Execution Spec Delta: ループコンテキスト履歴

## MODIFIED Requirements

### Requirement: Git Conflict Resolution

Git バックエンド使用時、システムは resolve コマンドの再試行時に前回の試行結果と継続理由をプロンプトに含めなければならない（MUST）。

resolve の目標（完了条件）は、少なくとも以下を満たすこととする：

- `git diff --name-only --diff-filter=U` が空である（未解決コンフリクトがない）
- Git マージが完了している（例: `MERGE_HEAD` が存在しない）
- 対象の各 `change_id` について、`Merge change: <change_id>` を含むマージコミットが存在する

上記の目標が満たされない場合、システムは継続理由を記録し、次回の `resolve_command` プロンプトに含めて再実行しなければならない（SHALL）。

#### Scenario: コンフリクト解消後もマージ未完了なら理由を伝えて継続

- **GIVEN** `git diff --name-only --diff-filter=U` が空である
- **AND** Git がマージ進行中状態である（例: `MERGE_HEAD` が存在する）
- **WHEN** `resolve_command` が成功終了する
- **THEN** システムは継続理由「Merge still in progress (MERGE_HEAD exists); retrying resolve」を記録する
- **AND** システムは次回の `resolve_command` プロンプトに前回の試行結果と継続理由を含める
- **AND** `resolve_command` を再実行する

#### Scenario: マージコミットが不足している場合は理由を伝えて継続

- **GIVEN** 対象の `change_id` のうち一部について `Merge change: <change_id>` を含むマージコミットが存在しない
- **WHEN** `resolve_command` が成功終了する
- **THEN** システムは継続理由「Missing merge commits for change_ids」と不足している ID リストを記録する
- **AND** システムは次回の `resolve_command` プロンプトに前回の試行結果と継続理由を含める
- **AND** `resolve_command` を再実行する

#### Scenario: Worktree マージ未完了なら理由を伝えて継続

- **GIVEN** 並列実行モードで resolve が実行されている
- **AND** worktree でマージが未完了（`MERGE_HEAD` が存在）
- **WHEN** `resolve_command` が成功終了する
- **THEN** システムは継続理由「Worktree merge still in progress for '{revision}'」を記録する
- **AND** システムは次回の `resolve_command` プロンプトに前回の試行結果と継続理由を含める
- **AND** `resolve_command` を再実行する

#### Scenario: Worktree コンフリクトが残っている場合は理由を伝えて継続

- **GIVEN** 並列実行モードで resolve が実行されている
- **AND** worktree でコンフリクトが残っている
- **WHEN** システムが検証を実行する
- **THEN** システムは継続理由「Worktree conflicts still present for '{revision}' ({files})」を記録する
- **AND** システムは次回の `resolve_command` プロンプトに前回の試行結果とコンフリクトファイルリストを含める
- **AND** `resolve_command` を再実行する

#### Scenario: Pre-sync コミットサブジェクト不正なら理由を伝えて継続

- **GIVEN** 並列実行モードで resolve が実行されている
- **AND** pre-sync マージコミットのサブジェクトが期待値「Pre-sync base into {change_id}」と異なる
- **WHEN** システムが検証を実行する
- **THEN** システムは継続理由「Invalid pre-sync merge commit subject in worktree '{revision}'」を記録する
- **AND** 期待されるサブジェクトと実際のサブジェクトを含める
- **AND** システムは次回の `resolve_command` プロンプトに前回の試行結果と継続理由を含める
- **AND** `resolve_command` を再実行する

#### Scenario: 最大試行回数後のエラーメッセージに全履歴が含まれる

- **GIVEN** resolve が最大試行回数に達した
- **AND** 目標がまだ満たされていない
- **WHEN** システムがエラーを報告する
- **THEN** エラーメッセージには試行回数が含まれる
- **AND** 最後の継続理由が含まれる

### Requirement: Archive Commit Completion via resolve_command

並列実行モードにおいて、archive 完了後のコミット作成は `resolve_command` に委譲し、再試行時には前回の試行結果をプロンプトに含めなければならない（SHALL）。

#### Scenario: Archive コミット作成の再試行時にコンテキストを含める

- **GIVEN** archive により `openspec/changes/{change_id}` が archive へ移動している
- **AND** 1回目の `resolve_command` 実行後も archive コミットが完了していない
- **WHEN** システムが2回目の `resolve_command` を実行する
- **THEN** プロンプトには前回の試行結果が含まれる
- **AND** 「Archive commit still incomplete」などの継続理由が含まれる
