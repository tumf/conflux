# vcs-worktree-operations Specification

## Purpose
TBD - created by archiving change add-worktree-view-with-merge. Update Purpose after archive.
## Requirements
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

### Requirement: Worktree setup script execution

システムは worktree 作成時にリポジトリ直下の `.wt/setup` スクリプトを検出し、存在する場合は実行しなければならない（MUST）。

セットアップ実行時、システムは環境変数 `ROOT_WORKTREE_PATH` にベースリポジトリ（ソースツリー）のパスを設定しなければならない（MUST）。

`.wt/setup` が存在しない場合、システムはセットアップ処理を実行してはならない（MUST NOT）。

#### Scenario: setupスクリプトが存在する場合に実行される
- **GIVEN** リポジトリ直下に `.wt/setup` が存在する
- **WHEN** 新しい worktree が作成される（TUIの「+」を含む）
- **THEN** `.wt/setup` が実行される
- **AND** `ROOT_WORKTREE_PATH` がベースリポジトリのパスとして設定される

#### Scenario: setupスクリプトが存在しない場合は何もしない
- **GIVEN** リポジトリ直下に `.wt/setup` が存在しない
- **WHEN** 新しい worktree が作成される（TUIの「+」を含む）
- **THEN** セットアップ処理は実行されない

#### Scenario: setupスクリプトが失敗した場合はエラーになる
- **GIVEN** `.wt/setup` が存在する
- **AND** スクリプトが非ゼロ終了コードで終了する
- **WHEN** 新しい worktree が作成される（TUIの「+」を含む）
- **THEN** worktree作成は失敗として扱われる
- **AND** 失敗理由がログに記録される
