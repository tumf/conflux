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

### Requirement: Worktree teardown script execution

The system MUST support an optional worktree-local `.wt/teardown` script that runs before deleting a Conflux-managed Git worktree.

When `<worktree-root>/.wt/teardown` exists and is executable, the system MUST execute it before `git worktree remove --force`.

- Execution context MUST use cwd=`<worktree-root>`
- `ROOT_WORKTREE_PATH` MUST be set to the repository root
- stdin MUST be null (`/dev/null`)

If teardown exits non-zero, worktree deletion MUST fail and MUST preserve the worktree for operator recovery.

If teardown is missing, deletion MUST proceed normally.

If teardown exists but is not executable, the system MUST skip teardown and continue deletion.

The system MUST expose an explicit skip-teardown deletion option for recovery operations. When skip-teardown is enabled, the system MUST bypass teardown execution and proceed with deletion while logging that teardown was skipped.

#### Scenario: Teardown runs before deletion when executable
- **GIVEN** target worktree contains executable `.wt/teardown`
- **WHEN** managed worktree deletion is requested
- **THEN** `.wt/teardown` is executed before `git worktree remove --force`
- **AND** cwd is target worktree root
- **AND** `ROOT_WORKTREE_PATH` is set
- **AND** stdin is null

#### Scenario: Teardown failure aborts deletion
- **GIVEN** target worktree contains executable `.wt/teardown`
- **AND** teardown exits non-zero
- **WHEN** managed worktree deletion is requested
- **THEN** deletion fails
- **AND** worktree is preserved for operator recovery

#### Scenario: Missing teardown proceeds with deletion
- **GIVEN** target worktree has no `.wt/teardown`
- **WHEN** managed worktree deletion is requested
- **THEN** deletion proceeds

#### Scenario: Non-executable teardown is skipped
- **GIVEN** target worktree contains non-executable `.wt/teardown`
- **WHEN** managed worktree deletion is requested
- **THEN** teardown is skipped
- **AND** deletion proceeds

#### Scenario: Explicit skip-teardown deletion proceeds
- **GIVEN** target worktree contains executable `.wt/teardown` that would fail
- **AND** deletion request enables `skip_teardown`
- **WHEN** managed worktree deletion is requested
- **THEN** teardown is not executed
- **AND** deletion proceeds
- **AND** skip behavior is logged

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

### Requirement: Worktree teardown script execution

The system MUST support an optional worktree-local `.wt/teardown` script that runs before deleting a Conflux-managed Git worktree.

When `<worktree-root>/.wt/teardown` exists and is executable, the system MUST execute it before invoking Git worktree removal. The teardown process MUST run non-interactively with the worktree root as its current working directory and MUST receive `ROOT_WORKTREE_PATH` pointing to the base repository root, matching the meaning used by `.wt/setup`.

`.wt/teardown` MUST be treated as a cleanup hook for side effects only. The system MUST NOT use teardown output or `.wt/state.env` as authoritative workflow-control state for resume routing, acceptance routing, archive routing, or next-action decisions.

#### Scenario: executable teardown runs before worktree removal

- **GIVEN** a Conflux-managed worktree contains executable `.wt/teardown`
- **WHEN** the system deletes that worktree
- **THEN** the system runs `.wt/teardown` before invoking `git worktree remove`
- **AND** the teardown current working directory is the worktree root
- **AND** `ROOT_WORKTREE_PATH` points to the base repository root
- **AND** teardown stdin is non-interactive

#### Scenario: teardown can read worktree-local state

- **GIVEN** a Conflux-managed worktree contains executable `.wt/teardown`
- **AND** the worktree contains `.wt/state.env`
- **AND** `.wt/teardown` reads `.wt/state.env` by relative path
- **WHEN** the system runs teardown before deletion
- **THEN** `.wt/teardown` can read the worktree-local state file from the worktree root

#### Scenario: missing teardown keeps existing deletion behavior

- **GIVEN** a Conflux-managed worktree does not contain `.wt/teardown`
- **WHEN** the system deletes that worktree
- **THEN** the system proceeds with Git worktree removal without running a teardown hook

#### Scenario: non-executable teardown is not run

- **GIVEN** a Conflux-managed worktree contains `.wt/teardown`
- **AND** `.wt/teardown` is not executable
- **WHEN** the system deletes that worktree
- **THEN** the system does not execute `.wt/teardown`
- **AND** the system records a warning or diagnostic that teardown was skipped because it was not executable
- **AND** the system proceeds with Git worktree removal

### Requirement: Teardown failure preserves worktree by default

If executable `.wt/teardown` exits unsuccessfully, the system MUST abort deletion by default before invoking Git worktree removal. The system MUST preserve the worktree and report diagnostics containing enough context to debug cleanup failure, including the worktree path, base repository path, exit status, stdout, and stderr when available.

#### Scenario: teardown failure aborts deletion

- **GIVEN** a Conflux-managed worktree contains executable `.wt/teardown`
- **AND** `.wt/teardown` exits with a non-zero status
- **WHEN** the system deletes that worktree without an explicit teardown skip option
- **THEN** the deletion fails before Git worktree removal
- **AND** the worktree remains available for operator recovery
- **AND** diagnostics include the teardown exit status, stdout, stderr, worktree path, and base repository path

### Requirement: Operators can explicitly skip teardown blocking cleanup

The system MUST provide an explicit operator escape hatch for managed worktree deletion that allows deletion to proceed without teardown blocking cleanup. The option SHOULD be named `skip_teardown` or `--skip-teardown` on exposed API/CLI/UI surfaces to avoid confusion with Git worktree removal's `--force` flag.

When this option is used, the system MUST record that teardown was skipped or that teardown failure was intentionally ignored before deletion continued.

#### Scenario: skip teardown proceeds with deletion

- **GIVEN** a Conflux-managed worktree contains executable `.wt/teardown`
- **AND** `.wt/teardown` would fail or require external resources that are unavailable
- **WHEN** an operator deletes the worktree with the explicit teardown skip option
- **THEN** the system proceeds with Git worktree removal
- **AND** the system records a warning or diagnostic that teardown did not block deletion because the skip option was used

### Requirement: Managed worktree deletion paths use teardown-aware removal consistently

All Conflux-managed worktree deletion paths MUST use teardown-aware removal unless a path is explicitly documented as out of scope for repository-defined teardown. This includes parallel execution cleanup, dependency-resolved stale worktree cleanup, inconsistent worktree cleanup, rejection cleanup, TUI manual deletion, server/WebUI worktree deletion, legacy web deletion, and proposal-session worktree deletion.

#### Scenario: orchestration cleanup runs teardown

- **GIVEN** a Conflux-managed worktree with executable `.wt/teardown` is being removed by an orchestration cleanup path
- **WHEN** cleanup is triggered after merge, rejection, dependency-resolved recreation, or inconsistent workspace detection
- **THEN** the cleanup path runs teardown-aware removal
- **AND** teardown failure prevents deletion by default unless that path explicitly uses the operator skip option

#### Scenario: manual and API deletion run teardown

- **GIVEN** a Conflux-managed worktree with executable `.wt/teardown` is deleted through TUI, WebUI, or server API
- **WHEN** the deletion request does not specify teardown skip
- **THEN** the manual or API deletion path runs teardown-aware removal
- **AND** teardown failure is surfaced to the operator instead of silently deleting the worktree
