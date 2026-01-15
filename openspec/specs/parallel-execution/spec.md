# parallel-execution Specification

## Purpose
Defines parallel change execution using jj workspaces or Git worktrees.
## Requirements
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

### Requirement: Parallel Event Bridge for TUI

The system SHALL provide a `ParallelEventBridge` that converts `ParallelEvent` to `OrchestratorEvent` for the TUI.

The bridge SHALL be a pure function with no side effects, enabling isolated testing.

#### Scenario: ApplyStarted event mapping

- **WHEN** a `ParallelEvent::ApplyStarted { change_id }` is received
- **THEN** the bridge SHALL return:
  - `OrchestratorEvent::Log(LogEntry::info("Apply started").with_change_id(&change_id))`
  - `OrchestratorEvent::ProcessingStarted(change_id)`

#### Scenario: ApplyCompleted event mapping

- **WHEN** a `ParallelEvent::ApplyCompleted { change_id, revision }` is received
- **THEN** the bridge SHALL return:
  - `OrchestratorEvent::Log(LogEntry::success("Apply completed").with_change_id(&change_id))`
  - `OrchestratorEvent::ProcessingCompleted(change_id)`

#### Scenario: ApplyFailed event mapping

- **WHEN** a `ParallelEvent::ApplyFailed { change_id, error }` is received
- **THEN** the bridge SHALL return:
  - `OrchestratorEvent::Log(LogEntry::error("Apply failed: {error}").with_change_id(&change_id))`
  - `OrchestratorEvent::ProcessingError { id: change_id, error }`

#### Scenario: ArchiveStarted event mapping

- **WHEN** a `ParallelEvent::ArchiveStarted { change_id }` is received
- **THEN** the bridge SHALL return:
  - `OrchestratorEvent::Log(LogEntry::info("Archiving...").with_change_id(&change_id))`
  - `OrchestratorEvent::ArchiveStarted(change_id)`

#### Scenario: ChangeArchived event mapping

- **WHEN** a `ParallelEvent::ChangeArchived { change_id }` is received
- **THEN** the bridge SHALL return:
  - `OrchestratorEvent::Log(LogEntry::success("Archived").with_change_id(&change_id))`
  - `OrchestratorEvent::ChangeArchived(change_id)`

#### Scenario: ArchiveFailed event mapping

- **WHEN** a `ParallelEvent::ArchiveFailed { change_id, error }` is received
- **THEN** the bridge SHALL return:
  - `OrchestratorEvent::Log(LogEntry::error("Archive failed: {error}").with_change_id(&change_id))`
  - `OrchestratorEvent::ProcessingError { id: change_id, error }`

### Requirement: Apply Loop Helper Functions

The system SHALL provide helper functions to separate concerns in the apply loop:

1. `check_task_progress(workspace_path, change_id)` - Reads and parses task progress
2. `summarize_output(output, max_lines)` - Formats command output for display

These helpers SHALL be pure functions where possible, enabling unit testing.

#### Scenario: Task progress check with valid file

- **GIVEN** a workspace with a valid `tasks.md` file at `openspec/changes/{change_id}/tasks.md`
- **WHEN** `check_task_progress()` is called
- **THEN** it SHALL return a `TaskProgress` with accurate `completed` and `total` counts

#### Scenario: Task progress check with missing file

- **GIVEN** a workspace without a `tasks.md` file
- **WHEN** `check_task_progress()` is called
- **THEN** it SHALL return a default `TaskProgress` with `completed=0` and `total=0`

#### Scenario: Output summarization

- **GIVEN** command output with 20 lines
- **WHEN** `summarize_output(output, 5)` is called
- **THEN** it SHALL return the last 5 lines prefixed with a line count indicator

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

### Requirement: Git Worktree Workspace Management

Git バックエンド使用時、システムは `git worktree` コマンドを使用してワークスペースを管理しなければならない（SHALL）。

#### Scenario: Create workspace with git worktree

- **WHEN** Git バックエンドでワークスペース作成が要求される
- **THEN** `git worktree add <path> -b <branch> <base_rev>` が実行される
- **AND** worktree ブランチ名は `{change_id}` と一致する
- **AND** 各変更は独立したブランチを持つ
- **AND** ワークスペースはdetached HEADであってはならない（MUST NOT）
- **AND** ワークスペースパスはシステム一時ディレクトリ配下に作成される

#### Scenario: Cleanup workspace

- **WHEN** Git ワークスペースのクリーンアップが要求される
- **THEN** `git worktree remove <path>` が実行される
- **AND** 関連ブランチ `git branch -D <branch>` が削除される

#### Scenario: Get workspace revision

- **WHEN** Git ワークスペースのリビジョンが要求される
- **THEN** `git rev-parse HEAD` の結果が返される

### Requirement: Git Clean Working Directory Requirement
When using the Git backend, the system SHALL warn about uncommitted changes and continue parallel execution.

#### Scenario: CLI warning on uncommitted changes
- **WHEN** the command runs with `--parallel`
- **AND** the Git backend is selected
- **AND** uncommitted or untracked files exist
- **THEN** the following warning message is displayed:
  ```
  Warning: Uncommitted changes detected.
  Parallel mode will continue, but uncommitted changes remain in your working directory.
  Consider committing or stashing if you need isolated workspaces.
  ```
- **AND** parallel execution starts
- **AND** the warning alone does not produce a non-zero exit code

#### Scenario: TUI warning on uncommitted changes
- **WHEN** F5 is pressed in the TUI
- **AND** the Git backend is selected
- **AND** uncommitted or untracked files exist
- **THEN** a popup dialog is displayed
- **AND** the title is "Uncommitted Changes Detected"
- **AND** the body explains the warning and that execution continues
- **AND** parallel execution starts

### Requirement: Git Sequential Merge

Git バックエンド使用時、システムは複数ブランチを逐次マージしなければならない（SHALL）。

逐次マージでは、各 change について以下の順序で統合を試みなければならない（SHALL）。

1. 事前同期: 統合先ブランチ（base）の最新を対象 worktree ブランチへ取り込む（base → worktree）
   - 事前同期でマージコミットが作成される場合、その subject は `Pre-sync base into <change_id>` の形式でなければならない（MUST）
2. 最終統合: 1 が完了した後、統合先ブランチへ対象 worktree ブランチをマージする（worktree → base）
   - 最終統合のマージコミット subject は `Merge change: <change_id>` の形式でなければならない（MUST）

ここでの `<change_id>` は対象ブランチに対応する **OpenSpec の change_id**（`openspec/changes/{change_id}`）と一致しなければならない（MUST）。

#### Scenario: Merge change_id は OpenSpec の change_id を使う

- **GIVEN** 逐次マージ対象の worktree ブランチと、それぞれに対応する OpenSpec の change_id が存在する
- **WHEN** `resolve_command` が逐次マージを完了する
- **THEN** 最終統合のマージコミット subject は `Merge change: <change_id>` の形式である
- **AND** （事前同期でマージコミットが作成される場合）その subject は `Pre-sync base into <change_id>` の形式である
- **AND** `change_id` は `openspec/changes/{change_id}` の ID と一致する

#### Scenario: 事前同期でコンフリクト解消を worktree 側で完結する

- **GIVEN** 対象 worktree ブランチの作成後に、統合先ブランチ（base）が更新されている
- **WHEN** システムが対象 change の逐次マージを開始する
- **THEN** システムはまず base → worktree の取り込みを試みる
- **AND** コンフリクトが発生した場合、コンフリクト解消は対象 worktree の作業コピーで行われる
- **AND** 事前同期が完了した後に worktree → base のマージが行われる
- **AND** 最終統合のマージコミット subject は `Merge change: <change_id>` である

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

### Requirement: Workspace Resume Detection

システムは並列実行開始時に、既存のworkspaceを検出しなければならない（SHALL）。

検出は `WorkspaceManager` traitの `find_existing_workspace(change_id)` メソッドにより行われる。

#### Scenario: Git worktree検出

- **WHEN** Gitバックエンドで並列実行が開始される
- **AND** 指定されたchange_idに対応するworktreeが存在する
- **AND** worktreeの現在ブランチが `{change_id}` である
- **AND** リポジトリ側に `refs/heads/{change_id}` が存在する
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

#### Scenario: worktreeとブランチが一致しない場合

- **WHEN** worktreeは存在するが現在ブランチが `{change_id}` ではない
- **OR** worktreeは存在するが `refs/heads/{change_id}` が存在しない
- **THEN** そのworktreeは再開対象として扱われない
- **AND** 既存worktree/ブランチは自動的に削除される
- **AND** 新規workspaceが作成される

### Requirement: Workspace Auto Resume

システムは既存workspaceを検出した場合、自動的に再利用しなければならない（SHALL）。
ただし、再利用は安全に一致判定できる場合に限られる（MUST）。

#### Scenario: 自動レジューム（デフォルト動作）

- **WHEN** 既存workspaceが検出される
- **AND** `--no-resume` フラグが指定されていない
- **AND** worktreeとブランチの整合が取れている
- **THEN** 確認なしで既存workspaceが自動的に再利用される
- **AND** ログに再利用の旨が出力される

#### Scenario: --no-resumeフラグ指定時

- **WHEN** `--no-resume` フラグが指定されている
- **AND** 既存workspaceが検出される
- **THEN** 既存workspaceは削除される
- **AND** 新規workspaceが作成される

### Requirement: WorkspaceInfo Structure

`WorkspaceInfo` 構造体は以下の情報を含まなければならない（SHALL）。

```rust
pub struct WorkspaceInfo {
    pub path: PathBuf,
    pub change_id: String,
    pub workspace_name: String,
    pub last_modified: SystemTime,
}
```

#### Scenario: WorkspaceInfo生成

- **WHEN** 既存workspaceが検出される
- **THEN** すべてのフィールドが適切に設定された `WorkspaceInfo` が返される
- **AND** `last_modified` はworkspaceディレクトリの最終更新時刻である

### Requirement: Workspace Reuse Flow

既存workspaceを再利用する場合、システムは適切な初期化を行わなければならない（SHALL）。

#### Scenario: Git worktree再利用

- **WHEN** Git worktreeの再利用が選択される
- **THEN** worktreeの状態が確認される
- **AND** 必要に応じて `git status` で状態が確認される
- **AND** apply loopが既存の進捗から継続される

### Requirement: TUI Resume Notification

TUIモードでは、既存workspace検出・再利用時に通知を表示しなければならない（SHALL）。

#### Scenario: TUIでの自動レジューム通知

- **WHEN** TUIモードで並列実行が開始される
- **AND** 既存workspaceが検出される
- **THEN** ログパネルに再利用メッセージが表示される
- **AND** メッセージには最終更新時刻が含まれる
- **AND** 確認ダイアログは表示されない（自動再開）

### Requirement: Workspace State Detection

システムは既存workspaceの再開時に、正確な状態を検出し、適切なアクションを実行しなければならない（SHALL）。

状態検出は `detect_workspace_state(change_id, workspace_path)` 関数により行われる。

#### Workspace States

ワークスペースは以下の5つの状態に分類される:

1. **Created**: ワークスペース作成直後、コミット履歴なし
2. **Applying**: WIPコミット存在（`WIP(apply): {change_id} (iteration N/M)`）
3. **Applied**: Applyコミット存在（`Apply: {change_id}`）
4. **Archived**: Archiveコミット存在（`Archive: {change_id}`）、mainブランチ未マージ
5. **Merged**: Archiveコミットがmainブランチにマージ済み

#### Scenario: Created状態の検出とアクション

- **WHEN** workspaceが検出される
- **AND** コミット履歴が存在しない
- **THEN** 状態は `WorkspaceState::Created` として判定される
- **AND** Apply処理が最初から開始される

#### Scenario: Applying状態の検出とアクション

- **WHEN** workspaceが検出される
- **AND** WIPコミット（`WIP(apply): {change_id} (iteration N/M)`）が存在する
- **THEN** 状態は `WorkspaceState::Applying { iteration: N }` として判定される
- **AND** Apply処理が次のイテレーション（N+1）から再開される

#### Scenario: Applied状態の検出とアクション

- **WHEN** workspaceが検出される
- **AND** Applyコミット（`Apply: {change_id}`）が存在する
- **AND** Archiveコミットが存在しない
- **THEN** 状態は `WorkspaceState::Applied` として判定される
- **AND** Apply処理はスキップされる
- **AND** Archive処理のみ実行される

#### Scenario: Archived状態の検出とアクション

- **WHEN** workspaceが検出される
- **AND** Archiveコミット（`Archive: {change_id}`）が存在する
- **AND** Archiveコミットがmainブランチにマージされていない
- **AND** working treeがクリーンである
- **THEN** 状態は `WorkspaceState::Archived` として判定される
- **AND** Apply/Archive処理はスキップされる
- **AND** Merge処理のみ実行される

#### Scenario: Merged状態の検出とアクション

- **WHEN** workspaceが検出される
- **AND** Archiveコミット（`Archive: {change_id}`）が存在する
- **AND** Archiveコミットがmainブランチにマージ済みである
- **THEN** 状態は `WorkspaceState::Merged` として判定される
- **AND** すべての処理（Apply/Archive/Merge）がスキップされる
- **AND** workspaceがクリーンアップされる

#### Scenario: 冪等性の保証

- **WHEN** 同じworkspaceで複数回実行される
- **THEN** 各実行時に適切な状態が検出される
- **AND** 完了済みの処理はスキップされる
- **AND** 最終的に同じ結果（Merged & Cleanup）に到達する

#### State Detection Functions

システムは以下の状態検出関数を提供しなければならない（SHALL）:

- `detect_workspace_state(change_id: &str, workspace_path: &Path) -> Result<WorkspaceState>`
  - メイン状態検出関数、上記5つの状態のいずれかを返す

- `is_merged_to_main(change_id: &str, repo_root: &Path) -> Result<bool>`
  - mainブランチにマージ済みかを判定

- `has_apply_commit(change_id: &str, repo_root: &Path) -> Result<bool>`
  - Applyコミットの存在を判定

- `get_latest_wip_snapshot(change_id: &str, repo_root: &Path) -> Result<Option<u32>>`
  - 最新のWIPイテレーション番号を取得

- `is_archive_commit_complete(change_id: &str, repo_root: &Path) -> Result<bool>`
  - Archiveコミットの存在とworking treeのクリーン状態を検証

### Requirement: Failed Change Tracking

並列実行において、失敗した変更を追跡し、依存する変更の実行判断に使用しなければならない（MUST）。

#### Scenario: Failed change recorded

- Given: 変更 `change-A` のapplyがエラーで終了した
- When: グループの実行が完了する
- Then: `change-A` は失敗した変更として記録される

#### Scenario: Failed change persists across groups

- Given: グループ1で `change-A` が失敗として記録された
- When: グループ2の実行が開始される
- Then: `change-A` は引き続き失敗した変更として追跡される

### Requirement: Dependent Change Skipping

失敗した変更に依存する変更は、自動的にスキップされなければならない（MUST）。

さらに、`MergeWait` により未統合の change を依存先に持つ変更は実行を保留し、今回の run では実行してはならない（MUST）。

#### Scenario: Dependent change skipped
- Given: `change-A` が失敗として記録されている
- And: `change-B` は `change-A` に依存している
- When: `change-B` の実行が開始されようとする
- Then: `change-B` はスキップされる
- And: `ChangeSkipped` イベントが発行される

#### Scenario: Independent change continues
- Given: `change-A` が失敗として記録されている
- And: `change-C` は `change-A` に依存していない
- When: `change-C` の実行が開始されようとする
- Then: `change-C` は通常通り実行される

#### Scenario: Skip reason logged
- Given: `change-B` が依存先 `change-A` の失敗によりスキップされる
- When: スキップが発生する
- Then: ログに「Skipping change-B because dependency change-A failed」が出力される

#### Scenario: `MergeWait` 依存の change はキューに残したまま実行しない
- **GIVEN** 変更 A が `MergeWait` であり base に未統合である
- **AND** 変更 B が変更 A に依存している
- **AND** 変更 B はキューに存在する
- **WHEN** 並列実行が次の実行対象を選定する
- **THEN** システムは変更 B を今回の run では実行しない
- **AND** 変更 B はキューから削除されない

### Requirement: ChangeSkipped Event

変更がスキップされた場合、`ChangeSkipped` イベントを発行しなければならない（MUST）。

#### Scenario: ChangeSkipped event emitted

- Given: `change-B` が依存先の失敗によりスキップされる
- When: スキップ処理が実行される
- Then: `ChangeSkipped { change_id: "change-B", reason: "Dependency 'change-A' failed" }` イベントが発行される

#### Scenario: TUI displays skipped change

- Given: TUIモードで実行中
- When: `ChangeSkipped` イベントを受信
- Then: ログペインに「Skipped: change-B (Dependency 'change-A' failed)」が表示される

### Requirement: Group Execution with Skip Check

グループ実行時に、各変更について依存先の失敗をチェックしなければならない（MUST）。

#### Scenario: Group execution with skip check

- Given: グループ2に `change-B`, `change-C` が含まれている
- And: `change-B` は失敗した `change-A` に依存している
- And: `change-C` は独立している
- When: グループ2の実行が開始される
- Then: `change-B` はスキップされる
- And: `change-C` のみが実行される

#### Scenario: All changes in group skipped

- Given: グループ内の全ての変更が失敗した依存先を持つ
- When: グループの実行が開始される
- Then: 全ての変更がスキップされる
- And: グループはエラーなく完了する（スキップはエラーではない）

### Requirement: Workspace Preservation on Error

並列実行においてエラーが発生した場合、workspaceを削除せずに保持しなければならない（MUST）。

#### Scenario: Workspace preserved on max iterations

- Given: applyループが最大イテレーション数に到達した
- When: エラーが発生する
- Then: workspaceは削除されず保持される
- And: `[ERROR] Failed for {change_id}, workspace preserved: {workspace_name}` がログ出力される

#### Scenario: Workspace preserved on apply failure

- Given: applyコマンドが非ゼロ終了コードで失敗した
- When: エラーが発生する
- Then: workspaceは削除されず保持される
- And: エラーログにworkspace名が含まれる

#### Scenario: Workspace cleaned up on success

- Given: changeの処理が正常に完了した（apply + archive）
- When: 処理完了後
- Then: workspaceは通常通りクリーンアップされる

#### Scenario: Resume hint logged on error

- Given: workspaceがエラーにより保持された
- When: エラーログが出力される
- Then: `[INFO] To resume: run with the same change_id, workspace will be automatically detected` が出力される

### Requirement: WorkspacePreserved Event

エラー時にworkspaceが保持された場合、TUIに通知するイベントを発行しなければならない（MUST）。

#### Scenario: WorkspacePreserved event emitted

- Given: エラーによりworkspaceが保持された
- When: クリーンアップフェーズがスキップされる
- Then: `ParallelEvent::WorkspacePreserved { change_id, workspace_name }` イベントが発行される

#### Scenario: TUI displays preserved workspace

- Given: TUIモードで実行中
- When: `WorkspacePreserved` イベントを受信
- Then: ログペインに「Workspace preserved: {workspace_name}」が表示される

### Requirement: Periodic Progress Commits

並列実行のapplyループにおいて、各イテレーション終了後に作業内容をスナップショットとして保存しなければならない（MUST）。進捗が増加しない場合でも、最新の作業内容をWIPコミットとして残さなければならない（MUST）。applyが失敗した場合でも、イテレーション終了時点の作業内容をWIPコミットとして残さなければならない（MUST）。

WIPコミットメッセージは `WIP: {change_id} ({completed}/{total} tasks, apply#{iteration})` の形式としなければならない（MUST）。WIPコミットは各イテレーションごとに新規コミットとして作成しなければならない（MUST）。既存WIPコミットの `--amend` を使用してはならない（MUST NOT）。

#### Scenario: Progress commit created after each successful apply
- Given: applyコマンドが正常に完了した
- When: イテレーションが終了する
- Then: WIPスナップショットが新規コミットとして作成される

#### Scenario: Snapshot created even when no progress made
- Given: applyコマンドが正常に完了したが、タスク進捗が増加しなかった
- When: イテレーションが終了する
- Then: 最新の作業内容を反映したWIPスナップショットが作成される

#### Scenario: Snapshot created after apply failure
- Given: applyコマンドが非ゼロ終了コードで失敗した
- When: イテレーションが終了する
- Then: 失敗時点の作業内容を反映したWIPスナップショットが作成される

#### Scenario: WIP message includes iteration index
- Given: WIPスナップショットを作成する
- When: コミットメッセージを設定する
- Then: メッセージに `apply#{iteration}` が含まれる

#### Scenario: Git backend snapshot handling
- Given: Gitバックエンドを使用している
- When: WIPスナップショットを作成する
- Then: `git add -A` と `git commit --allow-empty` 相当の操作で新規WIPコミットが作成される

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

### Requirement: Parallel execution completion status must accurately reflect actual processing outcome

The system SHALL send completion events and messages only when processing completes normally, not when stopped or cancelled by the user.

The system SHALL distinguish between successful completion, completion with errors, graceful stop, and cancellation.

**Priority**: HIGH
**Rationale**: Incorrect completion messages mislead users about the processing status and can cause confusion when resuming work.

#### Scenario: Graceful stop during parallel execution should not show success message

**Given** the orchestrator is running in parallel mode
**And** at least one change is queued for processing
**When** the user triggers graceful stop (ESC key) before any change completes
**Then** the orchestrator should stop processing
**And** should send `OrchestratorEvent::Stopped`
**And** should NOT send `OrchestratorEvent::AllCompleted`
**And** should NOT display "All parallel changes completed" message
**And** should NOT display "All changes processed successfully" message
**And** should display "Processing stopped" message only

#### Scenario: Force stop (cancel) during parallel execution should not show success message

**Given** the orchestrator is running in parallel mode
**And** at least one change is queued for processing
**When** cancellation is triggered via cancel token
**Then** the orchestrator should immediately stop
**And** should display "Parallel execution cancelled" message
**And** should NOT send `OrchestratorEvent::AllCompleted`
**And** should NOT display any success completion messages

#### Scenario: Successful parallel execution completion shows success message

**Given** the orchestrator is running in parallel mode
**And** multiple changes are queued for processing
**When** all changes complete successfully without errors
**Then** the orchestrator should send `OrchestratorEvent::AllCompleted`
**And** should display "All parallel changes completed" success message
**And** should display "All changes processed successfully" message

#### Scenario: Parallel execution with partial errors shows warning message

**Given** the orchestrator is running in parallel mode
**And** multiple changes are queued for processing
**When** at least one batch fails with an error
**And** the orchestrator continues processing remaining changes
**And** all queued changes have been attempted
**Then** the orchestrator should send `OrchestratorEvent::AllCompleted`
**And** should display "Processing completed with errors" warning message
**And** should NOT display "All changes processed successfully" message

### Requirement: Loop termination reason must be tracked and distinguished

The system SHALL track the reason for loop termination (cancellation, graceful stop, or normal completion) using local state flags.

The system SHALL use this information to conditionally send completion events and messages.

加えて、`merge_wait` を終了理由として区別し、成功完了と誤解される完了イベント/メッセージを送信してはならない（SHALL NOT）。

#### Scenario: Tracking stopped or cancelled state
- **Given** the parallel orchestration loop is running
- **When** the loop checks for cancellation or graceful stop
- **And** either condition is true
- **Then** a `stopped_or_cancelled` flag should be set to true
- **And** the loop should break
- **And** this flag should prevent sending completion events after the loop

#### Scenario: Tracking error state during batch processing
- **Given** the parallel orchestration loop is processing batches
- **When** a batch execution returns an error
- **Then** a `had_errors` flag should be set to true
- **And** processing should continue with remaining batches
- **And** this flag should affect the final completion message when all batches finish

#### Scenario: マージ待ちが残る場合は成功完了として扱わない
- **GIVEN** 並列実行で少なくとも 1 件の change が `MergeWait` で残っている
- **WHEN** 実行可能な queued change の処理が完了する
- **THEN** システムは `AllCompleted` 相当の成功完了を通知しない
- **AND** 停止/待機（merge 待ち）として扱われる

### Requirement: Parallel Execution with Hooks

parallel mode での実行時、システムは設定された hooks を適切なタイミングで実行しなければならない（SHALL）。

#### Scenario: Apply 前の hook 実行

- **GIVEN** `pre_apply` hook が設定されている
- **AND** parallel mode で change が処理されている
- **WHEN** apply コマンドが実行される前
- **THEN** `pre_apply` hook が実行される
- **AND** hook は workspace ディレクトリで実行される

#### Scenario: Archive 後の hook 実行

- **GIVEN** `post_archive` hook が設定されている
- **AND** parallel mode で change がアーカイブされる
- **WHEN** archive コマンドが成功した後
- **THEN** `post_archive` hook が実行される

#### Scenario: Hook 失敗時の動作（continue_on_failure = true）

- **GIVEN** `pre_apply` hook が設定されている
- **AND** `continue_on_failure = true` が設定されている
- **WHEN** hook の実行が失敗する
- **THEN** 警告がログに記録される
- **AND** apply コマンドは引き続き実行される

#### Scenario: Hook 失敗時の動作（continue_on_failure = false）

- **GIVEN** `pre_apply` hook が設定されている
- **AND** `continue_on_failure = false` が設定されている
- **WHEN** hook の実行が失敗する
- **THEN** change の処理がエラーで終了する
- **AND** 他の parallel change には影響しない

### Requirement: Parallel Hook Event Reporting

parallel mode での hook 実行は、`ParallelEvent` として報告されなければならない（SHALL）。

#### Scenario: Hook 開始イベント

- **GIVEN** parallel mode で hook が実行される
- **WHEN** hook の実行が開始される
- **THEN** `ParallelEvent::HookStarted` が発行される

#### Scenario: Hook 完了イベント

- **GIVEN** parallel mode で hook が実行される
- **WHEN** hook の実行が完了する
- **THEN** `ParallelEvent::HookCompleted` または `ParallelEvent::HookFailed` が発行される

### Requirement: Individual Merge on Archive Completion

並列実行モードにおいて、システムは各変更が archive 完了した時点で**即座に個別マージ**を実行しなければならない（SHALL）。

ただし、統合先ブランチ（base）が dirty（未コミット変更/未追跡ファイルの存在、または Git がマージ進行中状態）である場合、システムは個別マージを実行してはならない（SHALL NOT）。

この場合、システムは対象 change を `MergeWait` 状態として保持し、worktree をクリーンアップせずに維持しなければならない（SHALL）。

#### Scenario: Archive 完了後のマージに OpenSpec の change_id を適用する
- **GIVEN** 並列実行モードで変更 A が archive 完了した
- **AND** 変更 A の worktree ブランチ名と OpenSpec の change_id が取得できる
- **WHEN** archive 処理が完了する
- **THEN** システムは worktree ブランチ名をマージ対象として `resolve_command` を実行する
- **AND** マージコミットには OpenSpec の change_id が含まれる

#### Scenario: base が dirty のとき個別マージを延期する
- **GIVEN** 並列実行モードで変更 A が archive 完了した
- **AND** base ブランチが dirty（例: `git status --porcelain` が空ではない、または `MERGE_HEAD` が存在する）である
- **WHEN** システムが変更 A の個別マージを開始しようとする
- **THEN** システムは変更 A の個別マージを実行しない
- **AND** `ExecutionEvent::MergeDeferred` を発行する
- **AND** 変更 A は `MergeWait` として保持される

### Requirement: Parallel Execution Event Reporting

parallel 実行モジュールは、統一された `ExecutionEvent` 型を使用してイベントを発行しなければならない（SHALL）。

**変更内容**: `MergeCompleted` イベント受信時の TUI 側の状態遷移を `Archived` から **`Merged`** に変更する。これにより、並列モードでマージが完了した変更を明確に区別できる。

#### Scenario: マージ完了イベント受信時に Merged 状態に遷移

- **GIVEN** parallel executor が個別の change をマージする
- **WHEN** マージが成功する
- **THEN** `ExecutionEvent::MergeCompleted { change_id, revision }` が発行される
- **AND** TUIは `change_id` に該当する変更のステータスを **`Merged`** に設定する
- **AND** `Merged` 状態は "merged" として表示され、色は `Color::LightBlue` である

#### Scenario: 複数変更の逐次マージ時に各変更が Merged 状態に遷移

- **GIVEN** parallel executor が複数の change を逐次マージする
- **WHEN** 各変更のマージが成功する
- **THEN** 各変更ごとに `ExecutionEvent::MergeCompleted { change_id, revision }` が発行される
- **AND** 各変更のステータスが `Merged` に設定される
- **AND** TUI上で各変更が個別に "merged" として表示される

### Requirement: 並列モードのコミット起点対象判定
並列モードは、`HEAD` のコミットツリーに存在する change だけを実行対象として扱わなければならない（SHALL）。

並列実行の開始時、システムはコミットツリーから `openspec/changes/<change-id>/` を列挙し、対象外の change を除外しなければならない（SHALL）。

#### Scenario: 未コミット change を除外する
- **GIVEN** `HEAD` のコミットツリーに存在しない change がある
- **WHEN** 並列実行が開始される
- **THEN** その change は実行対象から除外される
- **AND** 除外された change ID が警告ログに記録される

### Requirement: 未コミット change の tasks 読み込みを行わない

並列モードは、**実行対象の判定**にコミットツリーを利用し、未コミット change を実行対象としてはならない（SHALL NOT）。

ただし、**進捗表示**については、worktree 内の未コミット `tasks.md` が存在する場合、それを優先的に読み取り、即座にユーザーに反映しなければならない（SHALL）。

#### Scenario: 未コミット change は実行対象外

- **GIVEN** `HEAD` のコミットツリーに存在しない未コミット change がある
- **WHEN** 並列モードの対象判定が行われる
- **THEN** その change は実行対象から除外される
- **AND** 除外された change ID が警告ログに記録される

#### Scenario: Worktree の未コミット tasks.md から進捗を読む

- **GIVEN** 並列実行中の change に対応する worktree が存在する
- **AND** worktree 内の `openspec/changes/{change_id}/tasks.md` が更新されている（未コミット）
- **WHEN** TUI の auto-refresh が実行される
- **THEN** システムは worktree 内の tasks.md を読み取る
- **AND** ベースツリーの tasks.md よりも worktree の内容が優先される
- **AND** TUI に即座に最新の進捗が表示される

#### Scenario: Worktree が存在しない場合のフォールバック

- **GIVEN** change に対応する worktree が存在しない
- **WHEN** 進捗の取得が試みられる
- **THEN** システムはベースツリーの `openspec/changes/{change_id}/tasks.md` から進捗を読み取る
- **AND** エラーは発生しない

#### Scenario: Worktree 読み取りエラー時の処理

- **GIVEN** worktree は存在するが tasks.md の読み取りに失敗する
- **WHEN** 進捗の取得が試みられる
- **THEN** システムは warning log を記録する
- **AND** ベースツリーから進捗を読み取る（silent fallback）
- **AND** TUI の表示には影響しない

### Requirement: Archive Commit Completion via resolve_command

並列実行モードにおいて、archive 完了後のコミット作成は `resolve_command` に委譲し、再試行時には前回の試行結果をプロンプトに含めなければならない（SHALL）。

#### Scenario: Archive コミット作成の再試行時にコンテキストを含める

- **GIVEN** archive により `openspec/changes/{change_id}` が archive へ移動している
- **AND** 1回目の `resolve_command` 実行後も archive コミットが完了していない
- **WHEN** システムが2回目の `resolve_command` を実行する
- **THEN** プロンプトには前回の試行結果が含まれる
- **AND** 「Archive commit still incomplete」などの継続理由が含まれる

### Requirement: Archive Resume Requires Archive Commit

resume 時に archive をスキップするのは、`Archive: <change_id>` コミットが存在し、かつ作業ツリーがクリーンである場合に限らなければならない（MUST）。

#### Scenario: Archive コミットが未完了なら resume で再コミットする

- **GIVEN** archive 済みの変更があり `openspec/changes/archive` に移動している
- **AND** `Archive: <change_id>` コミットが存在しない、または作業ツリーがクリーンではない
- **WHEN** resume が実行される
- **THEN** システムは `resolve_command` を再実行して archive コミットを完了させる

### Requirement: 衝突解決時のResolveStartedイベント送信

Parallel実行で衝突解決（conflict resolution）が開始される際、システムは対象 change_id を含む `ResolveStarted { change_id }` イベントを送信しなければならない（SHALL）。

これにより、TUI側で該当 change の状態を `QueueStatus::Resolving` に遷移させ、ユーザーに「どの change が解決中か」を視覚的に示すことができる。

#### Scenario: 自動衝突解決開始時にResolveStartedイベントを送信

- **GIVEN** parallel実行でmerge衝突が発生し、`resolve_conflicts_with_retry` が呼び出される
- **WHEN** 衝突解決が開始される直前
- **THEN** システムは対象 change_id を含む `ResolveStarted { change_id }` イベントを送信する
- **AND** TUIは該当 change の `queue_status` を `QueueStatus::Resolving` に遷移させる
- **AND** TUIには「resolving」ステータスが表示される

#### Scenario: 複数changeの順次マージで各changeにResolveStartedを送信

- **GIVEN** 複数の change を順次マージする `resolve_merges_with_retry` が実行される
- **WHEN** 各 change_id に対して衝突解決が開始される
- **THEN** 各 change_id ごとに `ResolveStarted { change_id }` イベントが送信される
- **AND** TUIでは対象 change が順番に「resolving」ステータスで表示される

#### Scenario: 解決完了時にResolveCompletedイベントを送信

- **GIVEN** 衝突解決が成功裏に完了する
- **WHEN** 解決処理が終了する
- **THEN** システムは `ResolveCompleted { change_id, worktree_change_ids }` イベントを送信する
- **AND** TUIは該当 change の `queue_status` を `QueueStatus::Archived` に遷移させる

#### Scenario: 解決失敗時にResolveFailedイベントを送信

- **GIVEN** 衝突解決が失敗する（最大リトライ回数到達など）
- **WHEN** 解決処理がエラーで終了する
- **THEN** システムは `ResolveFailed { change_id, error }` イベントを送信する
- **AND** TUIは該当 change の `queue_status` を `QueueStatus::MergeWait` に遷移させる
- **AND** エラーメッセージがTUIに表示される

### Requirement: キュー変更デバウンスとスロット駆動の再分析
並列実行中、システムはキュー変更（追加・削除）から10秒経過した後に再分析を行い、実行スロットが空いたタイミングで依存関係を考慮して次の変更を選定しなければならない（SHALL）。

#### Scenario: キュー変更後10秒経過かつスロット空きで再分析
- **GIVEN** 実行中にキュー変更が発生して最後の変更から10秒経過した
- **AND** 実行スロットが空いた（マージ完了またはエラー停止）
- **WHEN** 並列実行が次の候補を評価する
- **THEN** 依存関係分析結果を用いて次に実行可能なchangeを選定する

#### Scenario: スロット空きが先行した場合はデバウンス完了を待つ
- **GIVEN** 実行スロットが空いた
- **AND** 最後のキュー変更から10秒未満である
- **WHEN** 並列実行が次の候補を評価する
- **THEN** デバウンス完了まで再分析を行わず待機する

