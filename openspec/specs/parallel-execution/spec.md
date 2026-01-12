# parallel-execution Specification

## Purpose
Defines parallel change execution using jj workspaces or Git worktrees.
## Requirements
### Requirement: Shared Parallel Orchestration Service

The system SHALL provide a unified `ParallelRunService` that handles parallel change execution for both CLI and TUI modes.

The service SHALL accept a callback mechanism for event notifications, allowing different UI implementations to handle events appropriately.

The service SHALL encapsulate:
- jj availability checking
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

#### Scenario: Fallback to sequential execution

- **WHEN** jj is not available
- **THEN** the `ParallelRunService` SHALL fall back to sequential execution
- **AND** the caller SHALL be notified via an appropriate event

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

#### Scenario: JjWorkspaceManager implements trait

- **WHEN** jj リポジトリで並列実行が開始される
- **THEN** `JjWorkspaceManager` が `WorkspaceManager` trait を実装する
- **AND** 既存の jj ベースの並列実行動作は変更されない

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
1. jj リポジトリ（`.jj` ディレクトリ存在）→ jj バックエンド
2. Git リポジトリ（`.git` ディレクトリ存在）→ Git バックエンド
3. 両方なし → 並列実行不可エラー

#### Scenario: Auto-detect jj backend

- **WHEN** カレントディレクトリに `.jj` ディレクトリが存在する
- **AND** `--vcs` オプションが指定されていない、または `auto` である
- **THEN** jj バックエンドが選択される
- **AND** 既存の jj 並列実行動作が使用される

#### Scenario: Auto-detect git backend

- **WHEN** カレントディレクトリに `.jj` ディレクトリが存在しない
- **AND** `.git` ディレクトリが存在する
- **AND** `--vcs` オプションが指定されていない、または `auto` である
- **THEN** Git バックエンドが選択される

#### Scenario: No VCS available

- **WHEN** `.jj` も `.git` も存在しない
- **AND** `--parallel` フラグが指定されている
- **THEN** エラーメッセージ "Parallel mode requires jj or git repository" が表示される
- **AND** 終了コードは非ゼロである

#### Scenario: Explicit VCS selection with --vcs flag

- **WHEN** `--vcs git` が指定されている
- **AND** `.git` ディレクトリが存在する
- **THEN** Git バックエンドが使用される（jj が存在しても）

#### Scenario: Explicit VCS not available

- **WHEN** `--vcs jj` が指定されている
- **AND** `.jj` ディレクトリが存在しない
- **THEN** エラーメッセージ "jj repository not found (.jj directory missing)" が表示される
- **AND** 終了コードは非ゼロである

### Requirement: Git Worktree Workspace Management

Git バックエンド使用時、システムは `git worktree` コマンドを使用してワークスペースを管理しなければならない（SHALL）。

#### Scenario: Create workspace with git worktree

- **WHEN** Git バックエンドでワークスペース作成が要求される
- **THEN** `git worktree add <path> -b <branch> <base_rev>` が実行される
- **AND** 各変更は独立したブランチを持つ
- **AND** ワークスペースパスはシステム一時ディレクトリ配下に作成される

#### Scenario: Cleanup workspace

- **WHEN** Git ワークスペースのクリーンアップが要求される
- **THEN** `git worktree remove <path>` が実行される
- **AND** 関連ブランチ `git branch -D <branch>` が削除される

#### Scenario: Get workspace revision

- **WHEN** Git ワークスペースのリビジョンが要求される
- **THEN** `git rev-parse HEAD` の結果が返される

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

#### Scenario: jj behavior unchanged

- **WHEN** jj バックエンドが選択される
- **AND** 未コミット変更が存在する
- **THEN** 従来通り `jj new` でスナップショットが作成される
- **AND** 並列実行が正常に開始される
- **AND** エラーは発生しない

### Requirement: Git Sequential Merge

Git バックエンド使用時、システムは複数ブランチを逐次マージしなければならない（SHALL）。

#### Scenario: Merge single branch

- **WHEN** 1つのワークスペースブランチをマージする
- **THEN** `git merge <branch>` が実行される
- **AND** マージコミットが作成される

#### Scenario: Merge multiple branches sequentially

- **WHEN** 複数のワークスペースブランチをマージする
- **THEN** 各ブランチが1つずつマージされる
- **AND** マージ順序はワークスペース作成順である
- **AND** 各マージ後にコンフリクトがチェックされる

#### Scenario: Conflict detected during merge

- **WHEN** `git merge` がコンフリクトを検出する
- **THEN** `GitConflict` エラーが返される
- **AND** コンフリクトファイルのリストが含まれる
- **AND** AgentRunner によるコンフリクト解決が試行される

### Requirement: Git Conflict Resolution

Git バックエンド使用時、システムは Git コンフリクトマーカーを含む解決プロンプトを提供しなければならない（SHALL）。

#### Scenario: Git conflict resolution prompt

- **WHEN** Git マージでコンフリクトが発生する
- **THEN** AgentRunner に渡されるプロンプトに以下が含まれる:
  - "This project uses Git for version control, not jj."
  - コンフリクトファイルのリスト
  - Git コンフリクトマーカーの説明（`<<<<<<<`, `=======`, `>>>>>>>`）
  - `git status` の出力
  - 解決後の手順

#### Scenario: Resolution success

- **WHEN** AgentRunner がコンフリクトを解決する
- **AND** `git diff --name-only --diff-filter=U` が空を返す
- **THEN** マージが成功したと判断される
- **AND** 次のブランチのマージに進む

#### Scenario: Resolution failure after retries

- **WHEN** 最大リトライ回数（デフォルト3回）を超えてもコンフリクトが解決されない
- **THEN** エラーメッセージが表示される
- **AND** ワークスペースは保持される（手動検査用）

### Requirement: jj Merge Conflict Detection on Success

jj バックエンドで `jj new` によるマージコミット作成時、コマンドが成功ステータス（exit code 0）で終了した場合でもコンフリクトを検出し、適切なエラーを返さなければならない（MUST）。

jj は Git と異なり、コンフリクト状態のコミットを作成することが可能であり、`jj new` コマンドはコンフリクトがあっても成功ステータスで終了する。このため、コマンドの終了ステータスだけでなく、出力内容からコンフリクトを検出する必要がある。

#### Scenario: jj new が成功ステータスでコンフリクトを含む場合

- **WHEN** `jj new` コマンドが成功ステータス（exit code 0）で完了する
- **AND** stderr 出力に "conflict" または "Conflict" が含まれている
- **THEN** `VcsError::Conflict` エラーが返される
- **AND** `resolve_conflicts_with_retry` が呼び出される
- **AND** 設定された `resolve_command` が実行される

#### Scenario: jj new がコンフリクトなく成功する場合

- **WHEN** `jj new` コマンドが成功ステータス（exit code 0）で完了する
- **AND** stderr 出力にコンフリクト表示がない
- **THEN** マージコミットの revision ID が正常に返される
- **AND** `resolve_conflicts_with_retry` は呼び出されない

### Requirement: Workspace Resume Detection

システムは並列実行開始時に、既存のworkspaceを検出しなければならない（SHALL）。

検出は `WorkspaceManager` traitの `find_existing_workspace(change_id)` メソッドにより行われる。

#### Scenario: jj workspace検出

- **WHEN** jjバックエンドで並列実行が開始される
- **AND** 指定されたchange_idに対応するworkspaceが存在する
- **THEN** `WorkspaceInfo` が返される
- **AND** workspaceのパスと最終更新時刻が含まれる

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

### Requirement: Workspace Auto Resume

システムは既存workspaceを検出した場合、自動的に再利用しなければならない（SHALL）。

#### Scenario: 自動レジューム（デフォルト動作）

- **WHEN** 既存workspaceが検出される
- **AND** `--no-resume` フラグが指定されていない
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

#### Scenario: jj workspace再利用

- **WHEN** jj workspaceの再利用が選択される
- **THEN** workspaceの状態が確認される
- **AND** 必要に応じて `jj status` でworking copyが同期される
- **AND** apply loopが既存の進捗から継続される

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

