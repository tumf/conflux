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

