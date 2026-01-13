## MODIFIED Requirements
### Requirement: Shared Parallel Orchestration Service

The system SHALL provide a unified `ParallelRunService` that handles parallel change execution for both CLI and TUI modes using Git worktrees.

The service SHALL accept a callback mechanism for event notifications, allowing different UI implementations to handle events appropriately.

The service SHALL encapsulate:
- Git availability checking
- Change grouping by dependencies
- ParallelExecutor coordination
- Archiving of completed changes

#### Scenario: CLI uses ParallelRunService

- **WHEN** the CLI runs in parallel mode (`--parallel` flag)
- **THEN** the CLI SHALL use `ParallelRunService` to execute changes
- **AND** events SHALL be logged to stdout via the callback mechanism

#### Scenario: TUI uses ParallelRunService

- **WHEN** the TUI runs in parallel mode
- **THEN** the TUI SHALL use `ParallelRunService` to execute changes
- **AND** events SHALL be forwarded to the TUI event channel via the callback mechanism

#### Scenario: Parallel mode requires git repository

- **WHEN** parallel execution is requested
- **AND** a `.git` directory does not exist
- **THEN** `ParallelRunService` SHALL return an error indicating a git repository is required
- **AND** no parallel execution is started

### Requirement: VCS Backend Abstraction

システムは並列実行のために VCS バックエンド抽象化レイヤーを提供しなければならない（SHALL）。

`WorkspaceManager` trait は以下の操作を定義する:
- VCS 利用可能性チェック
- ワークスペース作成
- リビジョン取得
- マージ
- クリーンアップ
- 作業コピーのスナップショット
- コミットメッセージ設定
- ワークスペース内リビジョン取得
- VCS ステータス取得
- コンフリクト検出
- 緊急クリーンアップ（同期版）

#### Scenario: GitWorkspaceManager implements trait

- **WHEN** Git リポジトリで並列実行が開始される
- **THEN** `GitWorkspaceManager` が `WorkspaceManager` trait を実装する
- **AND** Git Worktree を使用してワークスペースを管理する

#### Scenario: ParallelExecutor uses trait object

- **WHEN** `ParallelExecutor` が初期化される
- **THEN** `workspace_manager` は `Box<dyn WorkspaceManager>` として保持される
- **AND** VCS バックエンドは設定または自動検出により決定される

### Requirement: VCS Backend Auto-Detection

システムは並列実行時に VCS バックエンドを自動検出しなければならない（SHALL）。

検出優先順位:
1. Git リポジトリ（`.git` ディレクトリ存在）→ Git バックエンド
2. `.git` が存在しない → 並列実行不可エラー

#### Scenario: Auto-detect git backend

- **WHEN** カレントディレクトリに `.git` ディレクトリが存在する
- **AND** `--vcs` オプションが指定されていない、または `auto` である
- **THEN** Git バックエンドが選択される

#### Scenario: No VCS available

- **WHEN** `.git` が存在しない
- **AND** `--parallel` フラグが指定されている
- **THEN** エラーメッセージ "Parallel mode requires git repository" が表示される
- **AND** 終了コードは非ゼロである

#### Scenario: Explicit VCS selection with --vcs flag

- **WHEN** `--vcs git` が指定されている
- **AND** `.git` ディレクトリが存在する
- **THEN** Git バックエンドが使用される

#### Scenario: Explicit VCS not available

- **WHEN** `--vcs git` が指定されている
- **AND** `.git` ディレクトリが存在しない
- **THEN** エラーメッセージ "git repository not found (.git directory missing)" が表示される
- **AND** 終了コードは非ゼロである

### Requirement: Git Clean Working Directory Requirement

Git バックエンド使用時、システムは未コミット変更がある場合に並列実行を拒否しなければならない（SHALL）。

#### Scenario: CLI error on uncommitted changes

- **WHEN** `--parallel` フラグで実行される
- **AND** Git バックエンドが選択される
- **AND** 未コミットまたは未追跡のファイルが存在する
- **THEN** 以下のエラーメッセージが表示される:
  ```
  Error: Cannot start parallel mode with uncommitted changes.

  Your working directory has uncommitted changes. Git worktree requires
  a clean working directory to create isolated workspaces.

  Please resolve this by either:

    1. Commit your changes:
       git add -A && git commit -m "WIP: save work before parallel"

    2. Stash your changes:
       git stash push -u -m "before parallel execution"

  Then run the command again.
  ```
- **AND** 終了コードは非ゼロである

#### Scenario: TUI popup error on uncommitted changes

- **WHEN** TUI で F5 キーが押される
- **AND** Git バックエンドが選択される
- **AND** 未コミットまたは未追跡のファイルが存在する
- **THEN** ポップアップダイアログが表示される
- **AND** タイトルは "Uncommitted Changes Detected" である
- **AND** 本文に解決手順が表示される
- **AND** Enter キーでダイアログを閉じることができる
- **AND** 並列実行は開始されない

### Requirement: Workspace Resume Detection

システムは並列実行開始時に、既存のworkspaceを検出しなければならない（SHALL）。

検出は `WorkspaceManager` traitの `find_existing_workspace(change_id)` メソッドにより行われる。

#### Scenario: Git worktree検出

- **WHEN** Gitバックエンドで並列実行が開始される
- **AND** 指定されたchange_idに対応するworktreeが存在する
- **THEN** `WorkspaceInfo` が返される
- **AND** worktreeのパスと最終更新時刻が含まれる

#### Scenario: workspaceが存在しない場合

- **WHEN** 指定されたchange_idに対応するworkspaceが存在しない
- **THEN** `None` が返される
- **AND** 新規workspaceが作成される

#### Scenario: 複数workspaceが存在する場合

- **WHEN** 指定されたchange_idに対応するworkspaceが複数存在する
- **THEN** 最終更新時刻（last_modified）が最も新しいworkspaceが選択される
- **AND** 選択されなかった古いworkspaceは自動的に削除される
- **AND** 削除処理のログが出力される

### Requirement: Workspace Reuse Flow

既存workspaceを再利用する場合、システムは適切な初期化を行わなければならない（SHALL）。

#### Scenario: Git worktree再利用

- **WHEN** Git worktreeの再利用が選択される
- **THEN** worktreeの状態が確認される
- **AND** 必要に応じて `git status` で状態が確認される
- **AND** apply loopが既存の進捗から継続される

### Requirement: Periodic Progress Commits

並列実行の apply ループにおいて、各イテレーション終了後に作業内容をスナップショットとして保存しなければならない（MUST）。進捗が増加しない場合でも、最新の作業内容を WIP コミットとして残さなければならない（MUST）。

WIP コミットメッセージは `WIP: {change_id} ({completed}/{total} tasks, apply#{iteration})` の形式としなければならない（MUST）。

#### Scenario: Progress commit created after each successful apply

- Given: apply コマンドが正常に完了した
- When: イテレーションが終了する
- Then: WIP スナップショットが作成される

#### Scenario: Snapshot created even when no progress made

- Given: apply コマンドが正常に完了したが、タスク進捗が増加しなかった
- When: イテレーションが終了する
- Then: 最新の作業内容を反映した WIP スナップショットが作成される

#### Scenario: WIP message includes iteration index

- Given: WIP スナップショットを作成する
- When: コミットメッセージを設定する
- Then: メッセージに `apply#{iteration}` が含まれる

#### Scenario: Git backend snapshot handling

- Given: Git バックエンドを使用している
- When: WIP スナップショットを作成する
- Then: `git add -A` と `git commit --allow-empty` 相当の操作で WIP コミットが作成される

### Requirement: Final Apply Squash

すべての apply イテレーションが成功した場合、システムは WIP スナップショットを単一の `Apply: {change_id} (apply#{final_iteration})` コミットに squash しなければならない（MUST）。apply が失敗した場合は squash を行ってはならない（MUST NOT）。

#### Scenario: Successful apply squashes WIP commits

- Given: apply ループが成功で終了した
- When: 最終処理が実行される
- Then: WIP コミットが 1 つの Apply コミットに統合される

#### Scenario: Apply commit includes final iteration index

- Given: Apply コミットを作成する
- When: コミットメッセージを設定する
- Then: `apply#{final_iteration}` が含まれる

#### Scenario: Failed apply preserves WIP commits

- Given: apply ループが失敗で終了した
- When: 終了処理が行われる
- Then: WIP コミットは保持され、squash は実行されない

#### Scenario: Git backend squash handling

- Given: Git バックエンドを使用している
- When: Apply コミットを作成する
- Then: `git reset --soft` と `git commit` 相当で WIP が統合される

## REMOVED Requirements
### Requirement: jj Merge Conflict Detection on Success
**Reason**: jj バックエンド廃止により対象外となるため。
**Migration**: Git コンフリクト検出と解決フローに統一する。

### Requirement: jj Workspace Merging
**Reason**: jj ワークスペースマージの仕様が不要になるため。
**Migration**: Git worktree のマージ仕様に一本化する。
