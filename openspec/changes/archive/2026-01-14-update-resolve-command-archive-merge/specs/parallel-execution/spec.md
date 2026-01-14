## MODIFIED Requirements

### Requirement: Git Sequential Merge

Git バックエンド使用時、システムは複数ブランチを逐次マージしなければならない（SHALL）。

各マージコミットメッセージは `Merge change: <change_id>` の形式とし、対象ブランチに対応する **OpenSpec の change_id**（`openspec/changes/{change_id}`）を含めなければならない（MUST）。

#### Scenario: Merge change_id は OpenSpec の change_id を使う

- **GIVEN** 逐次マージ対象の worktree ブランチと、それぞれに対応する OpenSpec の change_id が存在する
- **WHEN** `resolve_command` が逐次マージを完了する
- **THEN** 各マージコミットの subject は `Merge change: <change_id>` の形式である
- **AND** `change_id` は `openspec/changes/{change_id}` の ID と一致する

### Requirement: Individual Merge on Archive Completion

並列実行モードにおいて、システムは各変更が archive 完了した時点で**即座に個別マージ**を実行しなければならない（SHALL）。

#### Scenario: Archive 完了後のマージに OpenSpec の change_id を適用する

- **GIVEN** 並列実行モードで変更 A が archive 完了した
- **AND** 変更 A の worktree ブランチ名と OpenSpec の change_id が取得できる
- **WHEN** archive 処理が完了する
- **THEN** システムは worktree ブランチ名をマージ対象として `resolve_command` を実行する
- **AND** マージコミットには OpenSpec の change_id が含まれる

## ADDED Requirements

### Requirement: Archive Commit Completion via resolve_command

並列実行モードにおいて、archive 完了後のコミット作成は `resolve_command` に委譲しなければならない（SHALL）。

`resolve_command` は pre-commit がファイルを修正してコミットを中断する場合でも、再ステージと再コミットで archive コミットを完了させなければならない（SHALL）。

#### Scenario: Archive コミットが pre-commit 中断後に収束する

- **GIVEN** archive により `openspec/changes/{change_id}` が archive へ移動している
- **AND** `git commit -m "Archive: <change_id>"` が pre-commit により中断される
- **WHEN** `resolve_command` が archive コミットを完了させる
- **THEN** `git status --porcelain` は空である
- **AND** 直近コミットの subject は `Archive: <change_id>` である

### Requirement: Archive Resume Requires Archive Commit

resume 時に archive をスキップするのは、`Archive: <change_id>` コミットが存在し、かつ作業ツリーがクリーンである場合に限らなければならない（MUST）。

#### Scenario: Archive コミットが未完了なら resume で再コミットする

- **GIVEN** archive 済みの変更があり `openspec/changes/archive` に移動している
- **AND** `Archive: <change_id>` コミットが存在しない、または作業ツリーがクリーンではない
- **WHEN** resume が実行される
- **THEN** システムは `resolve_command` を再実行して archive コミットを完了させる
