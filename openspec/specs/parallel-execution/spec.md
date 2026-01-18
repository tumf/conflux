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

### Requirement: Parallel execution acceptance loop
Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace.
The acceptance loop SHALL parse stdout to determine pass/fail, and MUST NOT use exit code to determine acceptance verdict.
The acceptance prompt MUST include a hardcoded acceptance prompt followed by configured `acceptance_prompt`.
When resuming a workspace that has not completed archive, the orchestrator SHALL re-run acceptance before starting archive, even if tasks are already complete.

**Acceptance state persistence**: Acceptance results are NOT persisted to disk or git commits. Therefore, on resume:
- If workspace state is `Applying` or `Created`: Normal apply+acceptance loop proceeds
- If workspace state is `Applied`: Acceptance MUST be re-run before archive
- If workspace state is `Archiving` (archive files moved but not committed): Acceptance MUST be re-run before archive commit
- If workspace state is `Archived` or `Merged`: Acceptance is not required (archive already complete)

This ensures quality gates are always enforced, even after interruptions.

#### Scenario: Parallel acceptance success proceeds to archive
- **GIVEN** a change completes an apply iteration successfully in parallel mode
- **WHEN** acceptance output indicates PASS
- **THEN** the orchestrator proceeds to archive in that workspace

#### Scenario: Parallel acceptance failure returns to apply loop
- **GIVEN** a change completes an apply iteration successfully in parallel mode
- **WHEN** acceptance output indicates FAIL with findings
- **THEN** the orchestrator returns the change to the apply loop and records the findings

#### Scenario: Parallel acceptance command execution failure
- **GIVEN** a change completes an apply iteration successfully in parallel mode
- **WHEN** the acceptance_command exits with non-zero status
- **THEN** the orchestrator records the command failure and returns the change to the apply loop

#### Scenario: Resume forces acceptance before archive
- **GIVEN** a workspace is resumed after interruption
- **AND** archive is not yet complete for the change
- **WHEN** the orchestrator resumes processing
- **THEN** acceptance_command is executed before any archive command

### Requirement: Parallel apply runs in worktree
parallel mode の apply コマンドは、対象 change の worktree ディレクトリで実行しなければならない（MUST）。これにより base リポジトリの作業ツリーに直接変更が入らないようにする。worktree 以外のパス（base リポジトリなど）が指定された場合、システムはエラーとして扱い実行を中断しなければならない（MUST）。

#### Scenario: apply 実行が worktree 以外の場合は失敗する
- **GIVEN** parallel mode で change が実行対象に選ばれている
- **AND** apply 実行ディレクトリが worktree パスではない
- **WHEN** apply コマンドが実行される
- **THEN** システムはエラーを返し apply を停止する
- **AND** エラーメッセージに change_id と実行ディレクトリが含まれる

### Requirement: VCS Backend Abstraction

システムは並列実行のために VCS バックエンド抽象化レイヤーを提供しなければならない（SHALL）。

`WorkspaceManager` trait の `original_branch()` メソッドは、ベースブランチ名を返さなければならない（SHALL）。ベースブランチが未初期化の場合、`None` を返さなければならない（SHALL）。

システムは `original_branch()` が `None` を返す場合、適切なエラーメッセージとともに処理を中断しなければならない（SHALL）。ベースブランチとして特定の値（"main", "develop", "master" など）をハードコードしてはならない（MUST NOT）。

**変更理由**: ベースブランチを動的に取得する現在の設計を維持しつつ、フォールバックによるハードコードを排除し、明示的なエラーハンドリングを実現するため。

#### Scenario: original_branch returns current branch

- **WHEN** ワークスペースマネージャーが初期化される
- **AND** `get_current_branch()` が正常にブランチ名を返す
- **THEN** `original_branch()` はそのブランチ名を返す
- **AND** 返されるブランチ名は実行時のカレントブランチと一致する

#### Scenario: original_branch returns None before initialization

- **WHEN** ワークスペースマネージャーが作成されたが `create_worktree()` がまだ呼ばれていない
- **AND** `original_branch()` が呼ばれる
- **THEN** `None` を返す

#### Scenario: Error when original_branch is None during merge

- **WHEN** マージ処理が実行される
- **AND** `original_branch()` が `None` を返す
- **THEN** システムはエラーを返す
- **AND** エラーメッセージは "Original branch not initialized" を含む
- **AND** マージ処理は実行されない

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

#### Scenario: TUI warning on uncommitted changes
- **WHEN** F5 is pressed in the TUI
- **AND** the Git backend is selected
- **AND** uncommitted or untracked files exist
- **THEN** a warning message is logged in the TUI logs
- **AND** the warning is not shown as a popup dialog
- **AND** parallel execution starts

### Requirement: Git Sequential Merge

Git バックエンド使用時、システムは複数ブランチを逐次マージしなければならない（SHALL）。

マージ処理において、ターゲットブランチ（統合先ブランチ）は `original_branch()` から取得しなければならない（SHALL）。`original_branch()` が `None` を返す場合、システムはエラーを返さなければならない（SHALL）。

システムは、マージターゲットとして特定のブランチ名（"main", "develop" など）をハードコードしてはならない（MUST NOT）。

**システムは、すべてのマージ/Resolve 操作をプロセス全体で共有されるグローバルロックでシリアライズしなければならない（SHALL）。これにより、複数の `ParallelExecutor` インスタンスが存在する場合でも、base ブランチへのマージ操作が同時に実行されることを防ぐ。**

**変更理由**: 複数の `ParallelExecutor` インスタンスが独立したロックを持つことで、TUI や Run モードで Resolve 操作が同時に実行され、base ブランチの状態が競合する問題を防ぐため。

#### Scenario: Merge to dynamically determined branch

- **WHEN** Git バックエンドが複数ブランチのマージを実行する
- **AND** `original_branch()` が "develop" を返す
- **THEN** すべてのマージは "develop" ブランチに対して実行される
- **AND** "main" ブランチは参照されない

#### Scenario: Merge fails when original_branch is None

- **WHEN** システムがマージを開始しようとする
- **AND** `original_branch()` が `None` を返す
- **THEN** マージは実行されない
- **AND** エラーメッセージ "Original branch not initialized" が返される
- **AND** ユーザーにワークスペースの再作成を促す

#### Scenario: Merge verification uses original_branch

- **WHEN** システムがマージ後の検証を実行する
- **AND** `original_branch()` が "feature/main-work" を返す
- **THEN** 検証は "feature/main-work" ブランチに対するマージを確認する
- **AND** 他のブランチ（"main" など）は検証されない

#### Scenario: 複数インスタンスからの同時マージがグローバルロックでシリアライズされる

- **GIVEN** 2つの `ParallelExecutor` インスタンス A と B が存在する
- **AND** インスタンス A が `attempt_merge()` を実行中である
- **WHEN** インスタンス B が `attempt_merge()` を呼び出す
- **THEN** インスタンス B はグローバルロックの取得を待機する
- **AND** インスタンス A のマージが完了するまで B のマージは開始されない
- **AND** base ブランチへの変更が競合することはない

#### Scenario: TUI からの連続 Resolve がシリアライズされる

- **GIVEN** TUI モードで 2 つの deferred change A と B が存在する
- **AND** ユーザーが change A の resolve を開始する
- **WHEN** change A の resolve 中にユーザーが change B の resolve を開始する
- **THEN** change B の resolve はグローバルロック取得を待機する
- **AND** change A の resolve が完了してから change B の resolve が開始される
- **AND** Git の状態が競合することはない

### Requirement: Git Conflict Resolution

Git バックエンド使用時、システムは resolve コマンドの再試行時に前回の試行結果と継続理由をプロンプトに含めなければならない（MUST）。

resolve の目標（完了条件）は、少なくとも以下を満たすこととする：

- `git diff --name-only --diff-filter=U` が空である（未解決コンフリクトがない）
- Git マージが完了している（例: `MERGE_HEAD` が存在しない）
- 対象の各 `change_id` について、`Merge change: <change_id>` を含むマージコミットが存在する

resolve のプロンプトには、`--no-verify` を使用してはならない旨を明示しなければならない（MUST）。

上記の目標が満たされない場合、システムは継続理由を記録し、次回の `resolve_command` プロンプトに含めて再実行しなければならない（SHALL）。

#### Scenario: resolveプロンプトに--no-verify禁止を含める
- **WHEN** システムが resolve プロンプトを生成する
- **THEN** プロンプトに「`--no-verify` を使用しない」指示が含まれる

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

### Requirement: Parallel Analysis Targeting
並列実行のanalysisはqueuedのchangeのみを対象にしなければならない（MUST）。

実行中のchangeが存在せず、queuedのchangeも空の場合、システムはオーケストレーションを終了しなければならない（MUST）。

analysis対象をqueuedに限定するため、queuedに含まれないchange（例: merged済みchange、実行済みchange、削除済みchange）はanalysis対象から除外されなければならない（MUST）。

queuedのchangeが空の場合、analysisを実行してはならない（MUST）。

#### Scenario: queuedのみがanalysis対象になる
- **GIVEN** queuedにchangeが存在する
- **AND** queued以外に実行中のchangeが存在する
- **WHEN** 並列実行がanalysisを開始する
- **THEN** analysis対象はqueuedのchangeのみになる

#### Scenario: queued外のchangeはanalysis対象から除外される
- **GIVEN** queuedに含まれないchangeが存在する
- **AND** queuedには別のchangeが存在する
- **WHEN** 並列実行がanalysisを開始する
- **THEN** queued外のchangeはanalysis対象から除外される

#### Scenario: queuedが空ならanalysisを実行しない
- **GIVEN** queuedのchangeが存在しない
- **WHEN** 並列実行がanalysisを開始しようとする
- **THEN** analysisを実行しない

#### Scenario: 実行中とqueuedが空なら終了する
- **GIVEN** 実行中のchangeが存在しない
- **AND** queuedのchangeも空である
- **WHEN** 並列実行ループが次のanalysisを開始しようとする
- **THEN** analysisを実行しない
- **AND** オーケストレーションは完了状態になる

### Requirement: Workspace State Detection
既存workspaceの再開時に、archive 状態を以下の3段階で判定しなければならない（MUST）。

- archiving: `openspec/changes/archive/<date>-<change_id>` が worktree に存在するが、`Archive: <change_id>` のコミットが未完了
- archived: `Archive: <change_id>` のコミットが完了し、`openspec/changes/<change_id>` が存在しない
- merged: base ブランチに `Archive: <change_id>` が存在し、`openspec/changes/<change_id>` が存在しない

archiving の場合は apply を再実行せず、archive ループに進めなければならない（MUST）。

#### Scenario: worktreeにアーカイブ済みファイルがありコミットが未完了
- **GIVEN** worktree 内に `openspec/changes/archive/<date>-<change_id>` が存在する
- **AND** `Archive: <change_id>` のコミットが未完了である
- **WHEN** `detect_workspace_state(change_id, workspace_path, base_branch)` が呼ばれる
- **THEN** 状態は archiving と判定される
- **AND** apply ではなく archive ループに進む

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
- Then: `git add -A` と `git commit --no-verify --allow-empty` 相当の操作で新規WIPコミットが作成される

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

hook の実行は apply/archive の共通ループで扱われ、hook 実行と同じトランザクションでイベントを発行すること（SHALL）。

#### Scenario: Hook 開始イベント

- **GIVEN** parallel mode で hook が実行される
- **WHEN** hook の実行が開始される
- **THEN** `ParallelEvent::HookStarted` が発行される

#### Scenario: Hook 完了イベント

- **GIVEN** parallel mode で hook が実行される
- **WHEN** hook の実行が完了する
- **THEN** `ParallelEvent::HookCompleted` または `ParallelEvent::HookFailed` が発行される

#### Scenario: 共通ループからの hook イベント統一

- **GIVEN** parallel apply/archive の共通ループが hook 実行を担当する
- **WHEN** hook の実行が開始・完了・失敗する
- **THEN** すべての hook イベントは共通ループから発行される

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

#### Scenario: Worktreeが存在する場合はtasks.mdをworktree側からのみ読む
- **GIVEN** 並列実行中の change に対応する worktree が存在する
- **AND** worktree 内の `openspec/changes/{change_id}/tasks.md` が更新されている（未コミット）
- **WHEN** TUI の auto-refresh が実行される
- **THEN** システムは worktree 内の tasks.md を読み取る
- **AND** ベースツリーの tasks.md は参照されない

#### Scenario: Archived/Mergedの進捗もworktree側のarchive済みtasks.mdから読む
- **GIVEN** 並列実行中の change に対応する worktree が存在する
- **AND** worktree 内の `openspec/changes/archive/<date>-<change_id>/tasks.md` が更新されている（未マージ）
- **WHEN** TUI の auto-refresh が実行される
- **THEN** システムは worktree 内の archive 済み tasks.md を読み取る
- **AND** TUI の Archived/Merged 表示の進捗が更新される

### Requirement: Archive Commit Completion via resolve_command
archive ループに入る前に tasks.md の完了率が100%であることを検証し、未完了または欠落している場合は archive に進んではならない（MUST）。

#### Scenario: tasks.md が未完了の場合は archive を停止する
- **GIVEN** tasks.md の完了率が100%ではない
- **WHEN** archive が開始される
- **THEN** archive コマンドは実行されない
- **AND** エラーとして記録される

### Requirement: Individual Merge on Archive Completion

並列実行モードにおいて、システムは merge 実行前に `verify_archive_completion` を再検証し、`openspec/changes/{change_id}` が存在する場合は未アーカイブとして `MergeDeferred` を返して `MergeWait` に留めなければならない（SHALL）。

#### Scenario: Merge 直前に changes が残っている場合は MergeDeferred
- **GIVEN** 変更 A が archive 完了として処理された
- **AND** `openspec/changes/{change_id}` が存在している
- **WHEN** merge を開始する
- **THEN** `verify_archive_completion` は未アーカイブを返す
- **AND** `MergeDeferred` を返す
- **AND** 変更 A は `MergeWait` に留まる

### Requirement: Archive Resume Requires Archive Commit
archive コミットを確定する際、`ensure_archive_commit` は `openspec/changes/{change_id}` が存在する場合にエラーを返さなければならない（MUST）。

#### Scenario: changes 側が残っている場合は archive commit を作らない
- **GIVEN** `openspec/changes/{change_id}` が存在する
- **WHEN** `ensure_archive_commit` が archive コミットを作成しようとする
- **THEN** エラーを返す

### Requirement: 衝突解決時のResolveStartedイベント送信

Parallel実行で `MergeWait` の change をユーザーが resolve した場合、resolve 完了後に TUI は `Merged` 状態を表示しなければならない（SHALL）。

#### Scenario: `MergeWait` からの resolve 完了後に Merged を表示する
- **GIVEN** TUI の変更が `MergeWait` である
- **AND** ユーザーが `M` キーで resolve を開始する
- **WHEN** resolve が正常に完了する
- **THEN** `ExecutionEvent::MergeCompleted { change_id, revision }` が TUI に送信される
- **AND** TUI は該当 change のステータスを `Merged` に設定する

### Requirement: キュー変更デバウンスとスロット駆動の再分析
並列実行中、システムはキュー変更（追加・削除）を実行中でも監視し、変更から10秒経過した後に再分析を行い、実行スロットが空いたタイミングで依存関係を考慮して次の変更を選定しなければならない（SHALL）。

加えて、システムは再分析時に実行スロットの空き数を算出し、依存関係分析の `order`（依存関係を満たした上での推奨実行順序）に従って空き数分の change を同時に起動しなければならない（SHALL）。

依存関係は実行制約として扱い、`order` の上位にあっても依存先が base に Git マージされた状態（依存先の成果物を使って実行できる状態）になるまで開始してはならない（MUST）。

依存制約が解決した change は、依存解決後の実行開始時点で worktree を新規作成し、既存の worktree がある場合も作り直さなければならない（MUST）。この挙動は依存 change に固有であり、resume が常に成立することを保証しない前提の例外とする。

#### Scenario: 空きスロット数に応じて同時起動する
- **GIVEN** `max_concurrent_workspaces` が 3 に設定されている
- **AND** 依存関係が解決済みの change が 3 件以上ある
- **WHEN** 再分析が実行される
- **THEN** システムは空きスロット数に応じて最大 3 件まで同時に起動する
- **AND** 依存関係が未解決の change は起動しない

### Requirement: AI エージェントクラッシュリカバリー

Apply または Archive コマンドが異常終了（exit code ≠ 0）した場合、システムは自動的にリトライしなければならない（SHALL）。

リトライの動作は以下の通りとする：
- コマンドの終了ステータスを確認
- 終了ステータスが 0 以外の場合、リトライを試みる
- リトライ前に 2 秒間の待機を行う
- 最大リトライ回数に達した場合、エラーを返却する

Apply コマンドのリトライ回数は `max_apply_iterations` の値を使用する。
Archive コマンドのリトライ回数は `ARCHIVE_COMMAND_MAX_RETRIES` の値を使用する。

**変更理由**: parallel 実行でも CommandQueue 経由のリトライと stagger を適用し、serial と同等のクラッシュリカバリーを保証するため。

#### Scenario: Parallel apply でも自動リトライが有効になる

- **GIVEN** parallel mode で Apply コマンドが実行される
- **AND** `max_apply_iterations` が 3 に設定されている
- **WHEN** Apply コマンドが exit code 1 で異常終了する
- **THEN** システムは 2 秒待機後に Apply コマンドを再実行する
- **AND** リトライが完了するまで parallel の状態は Applying のまま維持される

### Requirement: Git 以外では WIP/スタール検知を無効化

WIP スナップショットとスタール検知は Git バックエンド時のみ有効とし、Git 以外のバックエンドではスキップしなければならない（MUST）。

#### Scenario: Git 以外では WIP スナップショットを作らない
- **GIVEN** Git 以外のバックエンドで apply ループが実行されている
- **WHEN** イテレーションが終了する
- **THEN** WIP スナップショットは作成されない
- **AND** スタール検知は実行されない

### Requirement: Parallel execution enforces workspace concurrency limit
システムは parallel 実行時、worktree 作成・apply・archive を含むすべての工程で `max_concurrent_workspaces` の上限を厳密に適用しなければならない（MUST）。これにより、同時に存在する worktree 数と同時実行される change 数が上限を超えないことを保証する。

#### Scenario: worktree 作成も同時数上限の対象になる
- **GIVEN** `max_concurrent_workspaces` が 3 に設定されている
- **AND** parallel 実行で 10 件の change が対象である
- **WHEN** worktree の作成と apply が進行する
- **THEN** 同時に作成・実行される worktree は最大 3 件までに制限される
- **AND** 残りの change はスロットが空くまで待機する
