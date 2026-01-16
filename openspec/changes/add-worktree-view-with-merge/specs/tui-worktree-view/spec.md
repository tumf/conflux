# tui-worktree-view Specification Delta

## Purpose
Worktree管理のための専用ビューを提供します。

## Relationship
- **Depends on**: `tui-architecture` (ビューとイベントシステムを使用)
- **Extends**: `tui-key-hints` (新しいキーバインドを追加)

## Requirements

## ADDED Requirements
### Requirement: Worktree View Mode

TUI SHALL provide a dedicated "Worktree View" mode for managing git worktrees.

#### Scenario: View mode切り替え

- **GIVEN** ユーザーがSelect、Running、またはStopped modeにいる
- **WHEN** Tabキーを押す
- **THEN** ChangesビューとWorktreeビューが切り替わる
- **AND** 現在のビューモードが視覚的に識別できる

#### Scenario: Worktreeビュー表示時のリスト取得

- **GIVEN** ユーザーがChangesビューにいる
- **WHEN** Tabキーを押してWorktreeビューに切り替える
- **THEN** `git worktree list --porcelain` が実行される
- **AND** worktreeリストが表示される
- **AND** 表示遅延が1秒未満である

#### Scenario: Worktreeビューからの復帰

- **GIVEN** ユーザーがWorktreeビューにいる
- **WHEN** Tabキーを押す
- **THEN** Changesビューに戻る
- **AND** 以前のカーソル位置が保持される

## ADDED Requirements
### Requirement: Worktree Information Display

Worktreeビュー SHALL display essential information for each worktree.

#### Scenario: Worktree基本情報の表示

- **GIVEN** git repositoryに複数のworktreeが存在する
- **WHEN** Worktreeビューを表示する
- **THEN** 各worktreeについて以下が表示される:
  - パスのbasename (例: "ws-feature-a")
  - ブランチ名 (例: "feature/new-ui")
  - Mainワークツリーの場合は "(main)" ラベル
  - Detached HEADの場合は "(detached)" 表示

#### Scenario: カーソル移動

- **GIVEN** Worktreeビューにworktreeが複数表示されている
- **WHEN** ↑↓キーまたはj/kキーを押す
- **THEN** カーソルが上下に移動する
- **AND** リストの端で折り返す (最上部から最下部へ、その逆も)

#### Scenario: 空のWorktreeリスト

- **GIVEN** git repositoryにworktreeが1つ (main) のみ存在する
- **WHEN** Worktreeビューを表示する
- **THEN** "(main)" worktreeのみが表示される
- **AND** "No additional worktrees" メッセージが表示される

## ADDED Requirements
### Requirement: Worktree Operations

Worktreeビュー SHALL provide operations to manage worktrees.

#### Scenario: Worktree削除

- **GIVEN** Worktreeビューで非mainのworktreeが選択されている
- **WHEN** Dキーを押す
- **THEN** 削除確認ダイアログが表示される
- **AND** Yキーで削除が実行される
- **AND** Nキーまたはescキーでキャンセルされる

#### Scenario: Main worktreeは削除不可

- **GIVEN** Worktreeビューでmain worktreeが選択されている
- **WHEN** Dキーを押す
- **THEN** "Cannot delete main worktree" 警告が表示される
- **AND** 削除ダイアログは表示されない

#### Scenario: Processing中のworktreeは削除不可

- **GIVEN** Running modeでchangeがProcessing状態
- **AND** そのchangeに関連するworktreeがWorktreeビューで選択されている
- **WHEN** Dキーを押す
- **THEN** "Cannot delete worktree: change 'xxx' is currently processing" 警告が表示される
- **AND** 削除ダイアログは表示されない

#### Scenario: Worktree作成

- **GIVEN** Worktreeビューが表示されている
- **AND** `worktree_command` が設定されている
- **WHEN** +キーを押す
- **THEN** 新しいworktreeが `ws-session-{timestamp}` 形式で作成される
- **AND** 新しいブランチ `ws-session-{random}` が作成される (detachedではなく)
- **AND** worktree_commandが実行される
- **AND** 実行後にworktreeリストが更新される

#### Scenario: エディタ起動

- **GIVEN** Worktreeビューでworktreeが選択されている
- **WHEN** eキーを押す
- **THEN** TUIが一時停止される
- **AND** 選択されたworktreeのルートディレクトリでエディタが起動される
- **AND** エディタ終了後にTUIが再開される

#### Scenario: シェル起動

- **GIVEN** Worktreeビューでworktreeが選択されている
- **AND** `worktree_command` が設定されている
- **WHEN** Enterキーを押す
- **THEN** TUIが一時停止される
- **AND** 選択されたworktreeディレクトリでworktree_commandが実行される
- **AND** コマンド終了後にTUIが再開される

#### Scenario: シェル起動 (設定なし)

- **GIVEN** Worktreeビューでworktreeが選択されている
- **AND** `worktree_command` が設定されていない
- **WHEN** Enterキーを押す
- **THEN** "No worktree_command configured" 警告が表示される
- **AND** シェルは起動されない

## ADDED Requirements
### Requirement: Dynamic Key Hints

Worktreeビュー SHALL display context-sensitive key hints.

#### Scenario: 基本キーヒント

- **GIVEN** Worktreeビューが表示されている
- **THEN** 以下のキーヒントが常に表示される:
  - "↑↓/jk: move"
  - "Tab: changes"
  - "+: create"

#### Scenario: 非mainワークツリー選択時のキーヒント

- **GIVEN** Worktreeビューで非mainのworktreeが選択されている
- **THEN** 追加で以下のキーヒントが表示される:
  - "D: delete"
  - "e: editor"
  - "Enter: shell" (worktree_command設定時のみ)

#### Scenario: Detached worktree選択時はマージヒント非表示

- **GIVEN** Worktreeビューでdetached HEADのworktreeが選択されている
- **THEN** "M: merge" キーヒントは表示されない

## MODIFIED Requirements
### Requirement: Changes View Key Hints

Changesビュー SHALL display key hints that exclude worktree operations and include view switching.

Worktree関連操作 (削除・作成) のキーヒントはWorktreeビューに移動され、Changesビューには表示されない。

#### Scenario: Changesビューからworktree操作キーを削除

- **GIVEN** Changesビューが表示されている
- **THEN** "D: delete WT" キーヒントは表示されない
- **AND** "+: worktree" キーヒントは表示されない
- **AND** "Tab: worktrees" キーヒントが表示される

#### Scenario: ChangesビューでD/+キーは無効

- **GIVEN** Changesビューでchangeが選択されている
- **WHEN** Dキーまたは+キーを押す
- **THEN** 何も起こらない (worktree削除・作成は実行されない)

## ADDED Requirements
### Requirement: Auto-Refresh Worktree List

Worktreeリスト SHALL be automatically refreshed.

#### Scenario: 定期的な自動更新

- **GIVEN** Worktreeビューが表示されている
- **WHEN** 5秒経過する
- **THEN** worktreeリストが自動的に再取得される
- **AND** 表示が更新される

#### Scenario: Worktree作成後の即時更新

- **GIVEN** Worktreeビューでworktreeを作成した
- **WHEN** worktree_commandが完了する
- **THEN** worktreeリストが即座に更新される
- **AND** 新しいworktreeが表示される

#### Scenario: Worktree削除後の即時更新

- **GIVEN** Worktreeビューでworktreeを削除した
- **WHEN** 削除が完了する
- **THEN** worktreeリストが即座に更新される
- **AND** 削除されたworktreeが表示から消える
