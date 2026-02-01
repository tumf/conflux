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

### Requirement: Worktree delete removes branch

When deleting a worktree from the Worktrees view, the system MUST also delete the associated local branch.

If the branch does not exist or deletion fails, the worktree deletion MUST still be treated as successful, and the branch deletion failure MUST be logged as a warning.

#### Scenario: Branch is deleted when worktree is deleted
- **GIVEN** A worktree deletion is executed from the Worktrees view
- **AND** The target worktree has an associated local branch
- **WHEN** The worktree deletion process completes
- **THEN** The local branch is also deleted
- **AND** Success logs for both worktree and branch deletion are recorded

#### Scenario: Worktree deletion succeeds even if branch deletion fails
- **GIVEN** A worktree deletion is executed from the Worktrees view
- **AND** The target branch has already been deleted
- **WHEN** The worktree deletion process completes
- **THEN** The worktree deletion is treated as successful
- **AND** A warning log for the branch deletion failure is recorded

### Requirement: worktree add のブランチ既存エラー分類

システムは `git worktree add` が「a branch named ... already exists」相当の stderr を返した場合、原因を「ブランチ既存」として分類しなければならない（MUST）。

#### Scenario: ブランチ既存エラーは分類される
- **GIVEN** `git worktree add` が「a branch named 'x' already exists」相当の stderr を返す
- **WHEN** worktree 作成に失敗する
- **THEN** 原因は「ブランチ既存」として分類される

### Requirement: ブランチ既存時の安全な worktree 再作成

`git worktree add <path> -b <branch> <base>` がブランチ既存で失敗した場合、システムは当該ブランチが他の worktree にチェックアウトされていないことを確認できたときに限り、`git worktree add <path> <branch>` を 1 回だけ再試行しなければならない（MUST）。

他の worktree にチェックアウト済みであることが確認できた場合、システムは再試行を行ってはならない（MUST NOT）。

#### Scenario: ブランチ既存かつ未チェックアウトなら再試行で成功する
- **GIVEN** `refs/heads/<branch>` は存在するが、どの worktree にもチェックアウトされていない
- **AND** `git worktree add <path> -b <branch> <base>` がブランチ既存で失敗する
- **WHEN** worktree 作成が再試行される
- **THEN** `git worktree add <path> <branch>` が 1 回だけ実行される

#### Scenario: ブランチ既存かつ他 worktree でチェックアウト済みなら再試行しない
- **GIVEN** `refs/heads/<branch>` が他の worktree でチェックアウトされている
- **AND** `git worktree add <path> -b <branch> <base>` がブランチ既存で失敗する
- **WHEN** worktree 作成が失敗する
- **THEN** 再試行は行われない

### Requirement: Worktree add failure diagnostics and safe retry

システムは `git worktree add` の失敗時に、stderr から代表的な原因を分類し、診断ログに含めなければならない（MUST）。

分類対象には最低限以下を含めなければならない（MUST）。
- 既存パス（worktree パスが既に存在）
- ブランチ重複（他の worktree でチェックアウト済み）
- 無効な参照（base commit / branch が存在しない）
- 権限エラー

`git worktree add` が既存パス起因で失敗した場合、システムは worktree 一覧に該当パスが存在しないことを確認できたときに限り、`git worktree prune` を実行し、1 回だけ再試行しなければならない（MUST）。

再試行後も失敗した場合、システムは元のエラーと分類結果の両方をログに残さなければならない（MUST）。

#### Scenario: 既存パスの失敗は分類される
- **GIVEN** `git worktree add` が「path already exists」相当の stderr を返す
- **WHEN** worktree 作成に失敗する
- **THEN** 原因は「既存パス」として分類される
- **AND** 分類結果が診断ログに含まれる

#### Scenario: ブランチ重複の失敗は分類される
- **GIVEN** `git worktree add` が「branch is already checked out」相当の stderr を返す
- **WHEN** worktree 作成に失敗する
- **THEN** 原因は「ブランチ重複」として分類される
- **AND** 分類結果が診断ログに含まれる

#### Scenario: 無効な参照の失敗は分類される
- **GIVEN** `git worktree add` が「invalid reference」相当の stderr を返す
- **WHEN** worktree 作成に失敗する
- **THEN** 原因は「無効な参照」として分類される
- **AND** 分類結果が診断ログに含まれる

#### Scenario: 権限エラーの失敗は分類される
- **GIVEN** `git worktree add` が「permission denied」相当の stderr を返す
- **WHEN** worktree 作成に失敗する
- **THEN** 原因は「権限エラー」として分類される
- **AND** 分類結果が診断ログに含まれる

#### Scenario: 既存パスで stale な worktree の場合は prune + 再試行
- **GIVEN** worktree パスが存在するが `git worktree list` に登録されていない
- **AND** `git worktree add` が既存パス起因で失敗する
- **WHEN** worktree 作成が再試行される
- **THEN** `git worktree prune` が実行される
- **AND** `git worktree add` は 1 回だけ再試行される

#### Scenario: 再試行が失敗した場合は元のエラーも保持する
- **GIVEN** `git worktree add` が既存パス起因で失敗する
- **AND** prune 後の再試行も失敗する
- **WHEN** エラーが記録される
- **THEN** 元のエラーと分類結果が両方ログに含まれる
