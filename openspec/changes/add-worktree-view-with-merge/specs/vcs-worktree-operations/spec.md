# vcs-worktree-operations Specification Delta

## Purpose
Git worktreeの低レベル操作とブランチマージ機能を提供します。

## Relationship
- **Extends**: VCS abstraction layer (git commands)
- **Used by**: `tui-worktree-view`, `tui-worktree-merge`

## Requirements

## ADDED Requirements
### Requirement: Git Worktree List

VCS layer SHALL provide functionality to list all git worktrees.

#### Scenario: Worktreeリスト取得

- **GIVEN** git repositoryに複数のworktreeが存在する
- **WHEN** `list_worktrees(repo_root)` を呼び出す
- **THEN** `git worktree list --porcelain` が実行される
- **AND** Vec<WorktreeInfo> が返される

#### Scenario: Porcelain形式のパース

- **GIVEN** `git worktree list --porcelain` の出力
- **WHEN** パーサーが実行される
- **THEN** 各worktreeについて以下が抽出される:
  - path (PathBuf)
  - head (commit hash)
  - branch (Option<String>)
  - is_detached (bool)
  - is_main (bool - 最初のworktree)

#### Scenario: 空のWorktreeリスト

- **GIVEN** git repositoryにworktreeが1つ (main) のみ存在する
- **WHEN** `list_worktrees(repo_root)` を呼び出す
- **THEN** 1つのWorktrееInfoが返される
- **AND** is_main が true である

## ADDED Requirements
### Requirement: Git Worktree Removal

VCS layer SHALL provide functionality to remove a worktree.

#### Scenario: Worktree削除

- **GIVEN** 削除対象のworktreeパスが存在する
- **WHEN** `worktree_remove(repo_root, worktree_path)` を呼び出す
- **THEN** `git worktree remove <path>` が実行される
- **AND** 成功時は Ok(()) が返される

#### Scenario: 存在しないWorktreeの削除

- **GIVEN** 指定されたパスにworktreeが存在しない
- **WHEN** `worktree_remove(repo_root, worktree_path)` を呼び出す
- **THEN** エラーが返される
- **AND** エラーメッセージにgitのstderrが含まれる

## ADDED Requirements
### Requirement: Merge Conflict Detection

VCS layer SHALL provide functionality to detect merge conflicts before merging.

#### Scenario: コンフリクトなしの検出

- **GIVEN** ブランチをマージしてもコンフリクトが発生しない
- **WHEN** `check_merge_conflicts(repo_root, branch_name)` を呼び出す
- **THEN** `git merge --no-commit --no-ff <branch>` が実行される
- **AND** `git merge --abort` で元に戻される
- **AND** Ok(None) が返される (コンフリクトなし)

#### Scenario: コンフリクトありの検出

- **GIVEN** ブランチをマージするとコンフリクトが発生する
- **WHEN** `check_merge_conflicts(repo_root, branch_name)` を呼び出す
- **THEN** `git merge --no-commit --no-ff <branch>` が実行される
- **AND** stderrから "CONFLICT" が検出される
- **AND** `git merge --abort` で元に戻される
- **AND** Ok(Some(MergeConflictInfo)) が返される

#### Scenario: コンフリクトファイルのパース

- **GIVEN** gitのstderrに複数のCONFLICTメッセージが含まれる
  ```
  CONFLICT (content): Merge conflict in src/main.rs
  CONFLICT (add/add): Merge conflict in README.md
  ```
- **WHEN** `parse_conflict_files(stderr)` を呼び出す
- **THEN** Vec<String> に ["src/main.rs", "README.md"] が含まれる

#### Scenario: Working directory汚れている場合

- **GIVEN** working directoryに未コミット変更がある
- **WHEN** `check_merge_conflicts(repo_root, branch_name)` を呼び出す
- **THEN** エラーが返される
- **AND** "Working directory must be clean to check merge conflicts" が含まれる

## ADDED Requirements
### Requirement: Branch Merge

VCS layer SHALL provide functionality to merge a branch.

#### Scenario: 通常マージ

- **GIVEN** working directoryがクリーン
- **AND** コンフリクトが発生しないブランチ
- **WHEN** `merge_branch(repo_root, branch_name)` を呼び出す
- **THEN** `git merge --no-ff --no-edit <branch>` が実行される
- **AND** マージコミットが作成される
- **AND** Ok(()) が返される

#### Scenario: コンフリクト発生時

- **GIVEN** マージ中にコンフリクトが発生する
- **WHEN** `merge_branch(repo_root, branch_name)` を呼び出す
- **THEN** `git merge --no-ff --no-edit <branch>` が失敗する
- **AND** `git merge --abort` が自動実行される
- **AND** エラーが返される
- **AND** エラーメッセージに "Merge conflict detected" が含まれる

#### Scenario: Working directoryクリーンチェック

- **GIVEN** working directoryに未コミット変更がある
- **WHEN** `merge_branch(repo_root, branch_name)` を呼び出す
- **THEN** エラーが返される
- **AND** "Working directory is not clean" が含まれる
- **AND** `git merge` は実行されない

## ADDED Requirements
### Requirement: Working Directory Status

VCS layer SHALL provide functionality to check working directory cleanliness.

#### Scenario: クリーンなWorking directory

- **GIVEN** working directoryに未コミット変更がない
- **WHEN** `is_working_directory_clean(repo_root)` を呼び出す
- **THEN** `git status --porcelain` が実行される
- **AND** Ok(true) が返される

#### Scenario: 汚れたWorking directory

- **GIVEN** working directoryに未コミット変更がある
- **WHEN** `is_working_directory_clean(repo_root)` を呼び出す
- **THEN** `git status --porcelain` が実行される
- **AND** Ok(false) が返される

## ADDED Requirements
### Requirement: Current Branch Detection

VCS layer SHALL provide functionality to get the current branch name.

#### Scenario: 通常ブランチの取得

- **GIVEN** repositoryが通常のブランチにある (例: main)
- **WHEN** `get_current_branch(repo_root)` を呼び出す
- **THEN** `git branch --show-current` が実行される
- **AND** Ok("main") が返される

#### Scenario: Detached HEADの検出

- **GIVEN** repositoryがdetached HEAD状態
- **WHEN** `get_current_branch(repo_root)` を呼び出す
- **THEN** `git branch --show-current` が空文字列を返す
- **AND** エラーが返される
- **AND** "Not on a branch (detached HEAD)" が含まれる

## ADDED Requirements
### Requirement: WorktreeInfo Type

WorktreeInfo struct SHALL represent git worktree metadata.

#### Scenario: WorktreeInfo構造

- **GIVEN** WorktreeInfo型が定義されている
- **THEN** 以下のフィールドを持つ:
  - path: PathBuf
  - head: String (commit hash)
  - branch: Option<String>
  - is_detached: bool
  - is_main: bool
  - merge_conflict: Option<MergeConflictInfo>

#### Scenario: display_label メソッド

- **GIVEN** WorktreeInfo { path: "/tmp/ws-feature", is_main: false, ... }
- **WHEN** display_label() を呼び出す
- **THEN** "ws-feature" が返される

#### Scenario: display_label (main)

- **GIVEN** WorktreeInfo { is_main: true, ... }
- **WHEN** display_label() を呼び出す
- **THEN** "(main)" が返される

#### Scenario: display_branch メソッド

- **GIVEN** WorktreeInfo { branch: Some("refs/heads/feature/new"), ... }
- **WHEN** display_branch() を呼び出す
- **THEN** "feature/new" が返される (refs/heads/ プレフィックス除去)

#### Scenario: display_branch (detached)

- **GIVEN** WorktreeInfo { branch: None, is_detached: true, ... }
- **WHEN** display_branch() を呼び出す
- **THEN** "(detached)" が返される

#### Scenario: has_merge_conflict メソッド

- **GIVEN** WorktreeInfo { merge_conflict: Some(...), ... }
- **WHEN** has_merge_conflict() を呼び出す
- **THEN** true が返される

#### Scenario: conflict_file_count メソッド

- **GIVEN** WorktreeInfo { merge_conflict: Some(MergeConflictInfo { conflicting_files: vec!["a.rs", "b.rs"], ... }), ... }
- **WHEN** conflict_file_count() を呼び出す
- **THEN** 2 が返される
