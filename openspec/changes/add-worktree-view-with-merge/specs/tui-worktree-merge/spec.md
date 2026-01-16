# tui-worktree-merge Specification Delta

## Purpose
Worktreeのブランチをbaseブランチにマージする機能を提供します。

## Relationship
- **Depends on**: `tui-worktree-view` (Worktreeビューを前提とする)
- **Depends on**: `vcs-worktree-operations` (マージとコンフリクト検出を使用)

## Requirements

## ADDED Requirements
### Requirement: Branch Merge from Worktree

Worktreeビュー SHALL provide the ability to merge a worktree's branch to the base branch.

#### Scenario: マージキーバインド

- **GIVEN** Worktreeビューでブランチを持つworktreeが選択されている
- **AND** コンフリクトが検出されていない
- **WHEN** Mキー (Shift+M) を押す
- **THEN** 選択されたworktreeのブランチがbase (現在のブランチ) にマージされる
- **AND** `git merge --no-ff --no-edit <branch>` が実行される

#### Scenario: マージ成功時のログ表示

- **GIVEN** マージが開始された
- **WHEN** マージが成功する
- **THEN** "Successfully merged branch 'xxx'" ログが表示される
- **AND** worktreeは削除されない (残る)

#### Scenario: マージ失敗時のエラー表示

- **GIVEN** マージが開始された
- **WHEN** マージが失敗する (例: working directoryが汚れている)
- **THEN** "Failed to merge branch 'xxx': <error>" ログが表示される
- **AND** エラー詳細がポップアップで表示される

#### Scenario: Detached worktreeはマージ不可

- **GIVEN** Worktreeビューでdetached HEADのworktreeが選択されている
- **WHEN** Mキーを押す
- **THEN** "Cannot merge detached HEAD. Please checkout a branch first." 警告が表示される
- **AND** マージは実行されない

#### Scenario: Main worktreeはマージ不可

- **GIVEN** Worktreeビューでmain worktreeが選択されている
- **WHEN** Mキーを押す
- **THEN** "Cannot merge from main worktree" 警告が表示される
- **AND** マージは実行されない

## ADDED Requirements
### Requirement: Pre-Merge Conflict Detection

Worktreeビュー SHALL detect merge conflicts before executing merge.

#### Scenario: コンフリクトなしのworktree表示

- **GIVEN** worktreeのブランチにコンフリクトがない
- **WHEN** Worktreeビューを表示する
- **THEN** そのworktreeは通常色で表示される
- **AND** "M: merge" キーヒントが表示される

#### Scenario: コンフリクトありのworktree表示

- **GIVEN** worktreeのブランチにコンフリクトがある
- **WHEN** Worktreeビューを表示する
- **THEN** そのworktreeは赤色で表示される
- **AND** "⚠{count}" バッジが表示される (countはコンフリクトファイル数)
- **AND** "M: merge" キーヒントは表示されない

#### Scenario: コンフリクトありでマージ試行

- **GIVEN** Worktreeビューでコンフリクトありのworktreeが選択されている
- **WHEN** Mキーを押す
- **THEN** "Cannot merge: N conflict(s) detected. Resolve conflicts before merging." 警告が表示される
- **AND** マージは実行されない

#### Scenario: コンフリクトチェック失敗時はマージ不可

- **GIVEN** worktreeのコンフリクトチェックが失敗した (例: gitコマンドエラー)
- **WHEN** Worktreeビューを表示する
- **THEN** そのworktreeは警告マークなしで表示される
- **AND** "M: merge" キーヒントは表示されない (マージ不可扱い)

#### Scenario: コンフリクトチェックの並列実行

- **GIVEN** Worktreeビューに複数の非mainワークツリーが存在する
- **WHEN** Worktreeビューを表示する
- **THEN** 各worktreeのコンフリクトチェックが並列実行される
- **AND** 全チェック完了まで1秒未満である

## ADDED Requirements
### Requirement: Merge Conflict Information

コンフリクト情報 SHALL be displayed clearly to users.

#### Scenario: コンフリクトバッジの形式

- **GIVEN** worktreeに3つのファイルでコンフリクトがある
- **WHEN** Worktreeビューを表示する
- **THEN** そのworktreeの右に "⚠3" が赤色・太字で表示される

#### Scenario: コンフリクトの自動更新

- **GIVEN** Worktreeビューが表示されている
- **WHEN** 5秒の自動リフレッシュが実行される
- **THEN** コンフリクト状態が再チェックされる
- **AND** コンフリクトバッジが更新される

## ADDED Requirements
### Requirement: Merge Pre-conditions

マージ実行前 SHALL validate all pre-conditions.

#### Scenario: Working directoryクリーンチェック

- **GIVEN** base repositoryのworking directoryに未コミット変更がある
- **WHEN** マージを実行しようとする
- **THEN** "Working directory is not clean. Commit or stash changes before merging." エラーが表示される
- **AND** マージは実行されない

#### Scenario: Base branchの取得

- **GIVEN** マージが開始された
- **WHEN** マージ処理が実行される
- **THEN** `git branch --show-current` でbase branchが取得される
- **AND** ログに "Merging 'feature-x' into 'main' with --no-ff" が表示される

#### Scenario: Detached HEADのbase

- **GIVEN** base repositoryがdetached HEAD状態
- **WHEN** マージを実行しようとする
- **THEN** "Not on a branch (detached HEAD)" エラーが表示される
- **AND** マージは実行されない

## ADDED Requirements
### Requirement: Merge Error Handling

マージ失敗時 SHALL provide clear error messages and recovery guidance.

#### Scenario: コンフリクト発生時の自動abort

- **GIVEN** マージ中にコンフリクトが発生した
- **WHEN** `git merge` がコンフリクトを報告する
- **THEN** `git merge --abort` が自動実行される
- **AND** "Merge conflict detected. Merge aborted. Manual resolution required." エラーが表示される
- **AND** コンフリクトファイル一覧が含まれる

#### Scenario: エラーポップアップの表示

- **GIVEN** マージが失敗した
- **WHEN** エラーイベントが受信される
- **THEN** エラー詳細がポップアップで表示される
- **AND** ポップアップは任意のキーで閉じられる

#### Scenario: マージ失敗後の状態

- **GIVEN** マージが失敗した
- **WHEN** エラー処理が完了する
- **THEN** base repositoryの状態は変更されていない
- **AND** worktreeは削除されていない
- **AND** ユーザーは再試行または手動解決を選択できる
