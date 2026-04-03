# parallel-execution Specification

## Purpose
Defines parallel change execution using jj workspaces or Git worktrees.
## Requirements
### Requirement: Shared Parallel Orchestration Service
システムはCLIとTUIの並列実行を扱う統一的な`ParallelRunService`を提供しなければならない（SHALL）。

サービスはイベント通知のためのコールバック機構を受け取り、TUIへ送るイベントは共有状態の更新より先に送信しなければならない（MUST）。これによりUI更新が共有状態のロック待ちで遅延しない。

サービスは以下をカプセル化すること：
- Git availability checking
- Change grouping by dependencies
- ParallelExecutor coordination
- Archiving of completed changes

ParallelRunService は、コミットツリーに存在しない change の除外と警告通知を CLI/TUI のどちらの経路でも同一ロジックで実行しなければならない（SHALL）。

#### Scenario: CLI uses ParallelRunService
- **WHEN** the CLI runs in parallel mode (`--parallel` flag)
- **THEN** the CLI SHALL use `ParallelRunService` to execute changes
- **AND** events SHALL be logged to stdout via the callback mechanism

#### Scenario: TUI uses ParallelRunService
- **WHEN** the TUI runs in parallel mode
- **THEN** the TUI SHALL use `ParallelRunService` to execute changes
- **AND** events SHALL be forwarded to the TUI event channel via the callback mechanism
- **AND** event forwarding happens before shared state updates so Accepting can render promptly

#### Scenario: TUI event forwarding precedes shared state update
- **GIVEN** `ParallelEvent::AcceptanceStarted` is processed by the forwarder
- **WHEN** the event is forwarded to the TUI
- **THEN** the TUI event channel receives the event before the shared state write lock is acquired
- **AND** the change status can transition to `Accepting` while acceptance is running

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
The acceptance loop SHALL parse stdout to determine pass/fail/continue/blocked, and MUST NOT use exit code to determine acceptance verdict.
The acceptance prompt MUST include a hardcoded acceptance prompt followed by configured `acceptance_prompt`.
When resuming a workspace that has not completed archive, the orchestrator SHALL re-run acceptance before starting archive, even if tasks are already complete.

**Acceptance state persistence**: Acceptance results are NOT persisted to disk or git commits. Therefore, on resume:
- If workspace state is `Applying` or `Created`: Normal apply+acceptance loop proceeds
- If workspace state is `Applied`: Acceptance MUST be re-run before archive
- If workspace state is `Archiving` (archive files moved but not committed): Acceptance MUST be re-run before archive commit
- If workspace state is `Archived` or `Merged`: Acceptance is not required (archive already complete)

This ensures quality gates are always enforced, even after interruptions.

- The second and later acceptance attempts MUST focus on the updated file list since the previous acceptance attempt and the previously reported findings, rather than performing a full re-check.
- The acceptance prompt for second and later attempts MUST include the updated file list (file paths only) since the previous acceptance attempt.
- The acceptance prompt for second and later attempts MUST include the previous acceptance findings and instruct the agent to verify whether those findings are resolved.
- The acceptance prompt for second and later attempts MUST instruct the agent to read relevant files as needed; it MUST NOT include diff content.
- Acceptance failures SHALL record findings using stdout/stderr tail lines without parsing `FINDINGS:` structure.
- Acceptance findings MUST exclude `ACCEPTANCE:` markers and the `FINDINGS:` header line from the recorded tail lines.
- Acceptance FAIL logs MUST NOT label tail line counts as "findings"; if counts are shown, they MUST be labeled as tail lines.
- If acceptance output is BLOCKED, the orchestrator MUST stop apply retries for the change and preserve the workspace for manual follow-up.
- If acceptance output is BLOCKED, the change MUST be recorded as a terminal failure for dependency skipping in the current run.

#### Scenario: Parallel acceptance retry narrows to updated files and prior findings
- **GIVEN** a change completes an apply iteration successfully in parallel mode
- **AND** acceptance output indicates CONTINUE
- **WHEN** the orchestrator runs a subsequent acceptance attempt for the same change
- **THEN** the acceptance prompt includes only the updated file list since the previous acceptance attempt (no diff content)
- **AND** the acceptance prompt includes the prior acceptance findings for verification
- **AND** the acceptance prompt instructs the agent to read files as needed to confirm fixes

#### Scenario: Parallel acceptance failure logging uses tail lines
- **GIVEN** acceptance output tail includes `ACCEPTANCE: FAIL` and `FINDINGS:` lines
- **WHEN** the orchestrator records the acceptance failure
- **THEN** the recorded findings exclude the acceptance markers and `FINDINGS:` header
- **AND** logs do not report "N findings" based on tail line count

#### Scenario: Acceptance blocked preserves workspace and stops apply
- **GIVEN** acceptance output indicates `ACCEPTANCE: BLOCKED`
- **WHEN** the orchestrator processes the acceptance result
- **THEN** the workspace is preserved for manual follow-up
- **AND** apply retries for the change are stopped in the current run

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

resolve の最終マージは `git merge --no-ff --no-commit <branch>` で開始し、コミット前に以下を実行するようプロンプトで指示しなければならない（MUST）：

- `openspec/changes/{change_id}/proposal.md` が存在し、かつ `openspec/changes/archive/` 配下に同一 `change_id` のアーカイブが存在する場合、`openspec/changes/{change_id}` を削除する
- 削除後に `git add -A` を実行し、`git commit -m "Merge change: <change_id>"` で同一マージコミットを作成する

上記の目標が満たされない場合、システムは継続理由を記録し、次回の `resolve_command` プロンプトに含めて再実行しなければならない（SHALL）。

#### Scenario: resolve プロンプトが no-commit と復活削除手順を含む
- **WHEN** システムが resolve プロンプトを生成する
- **THEN** プロンプトに `git merge --no-ff --no-commit <branch>` が含まれる
- **AND** プロンプトに `openspec/changes/{change_id}` の復活検知と削除手順が含まれる

#### Scenario: 復活した changes はマージコミット前に削除される
- **GIVEN** `openspec/changes/{change_id}/proposal.md` が存在する
- **AND** `openspec/changes/archive/` 配下に同一 `change_id` のアーカイブが存在する
- **WHEN** resolve の最終マージ手順を実行する
- **THEN** `openspec/changes/{change_id}` は `git commit -m "Merge change: <change_id>"` の前に削除される

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

re-analysis は完了イベントに依存せず、キュー変化やタイマーなどのトリガで起動可能でなければならない（MUST）。

re-analysis はメインの実行ループ進行に依存せず開始できなければならない（MUST）。

スロットが空いていない場合でも re-analysis は実行でき、空きができた時点で次のディスパッチが行われなければならない（MUST）。

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

#### Scenario: キュー変化でre-analysisが起動する
- **GIVEN** 実行中のchangeが存在する
- **AND** queuedにchangeが追加される
- **WHEN** 並列実行がre-analysisを評価する
- **THEN** 完了イベントを待たずにre-analysisが開始される
- **AND** メインの実行ループ進行に依存しない

#### Scenario: スロットが空いていない場合でもre-analysisできる
- **GIVEN** 利用可能なスロットが0である
- **AND** queuedにchangeが存在する
- **WHEN** 並列実行がre-analysisを開始する
- **THEN** re-analysisは実行される
- **AND** スロットが空いた時点で次のchangeがディスパッチされる

### Requirement: Workspace State Detection
既存workspaceの再開時に、archive 状態をコミットメッセージではなく **コミットされたファイルの状態** で判定しなければならない（MUST）。

判定基準（すべて worktree HEAD ツリーのファイル状態で判定）:

- archiving: worktree が dirty（未コミットの変更がある）かつ `openspec/changes/<change_id>` が存在せず、archive エントリ（`openspec/changes/archive/<date>-<change_id>` または `openspec/changes/archive/<change_id>`）が存在する
- archived: worktree が clean であり、`openspec/changes/<change_id>` が存在せず、archive エントリが存在する
- merged: base ブランチの HEAD ツリーに archive エントリが存在し、`openspec/changes/<change_id>` が存在しない

archiving の場合は apply を再実行せず、archive ループに進めなければならない（MUST）。
archived の場合は apply/archive を再実行せず、merge のみ実行しなければならない（MUST）。

#### Scenario: worktreeがdirtyでarchiveエントリがあればarchiving
- **GIVEN** worktree 内の `openspec/changes/<change_id>` が存在しない
- **AND** worktree 内に `openspec/changes/archive/<date>-<change_id>` が存在する
- **AND** worktree が dirty である（未コミットの変更がある）
- **WHEN** `detect_workspace_state(change_id, workspace_path, base_branch)` が呼ばれる
- **THEN** 状態は archiving と判定される
- **AND** apply ではなく archive ループに進む

#### Scenario: worktreeがcleanでarchiveエントリがあればarchived
- **GIVEN** worktree が clean である
- **AND** worktree HEAD ツリーに `openspec/changes/test-change` が存在しない
- **AND** worktree HEAD ツリーに `openspec/changes/archive/2024-01-15-test-change` が存在する
- **WHEN** `detect_workspace_state(test-change, workspace_path, base_branch)` が呼ばれる
- **THEN** 状態は archived と判定される
- **AND** apply/archive を再実行せず merge のみ実行する

#### Scenario: baseブランチにarchiveエントリがあればmerged
- **GIVEN** base ブランチの HEAD ツリーに `openspec/changes/archive/2024-01-15-test-change` が存在する
- **AND** base ブランチの HEAD ツリーに `openspec/changes/test-change` が存在しない
- **WHEN** `detect_workspace_state(test-change, workspace_path, base_branch)` が呼ばれる
- **THEN** 状態は merged と判定される

### Requirement: Failed Change Tracking
並列実行において、失敗した変更を追跡し、依存する変更の実行判断に使用しなければならない（MUST）。

権限auto-rejectなど人手介入が必要なapply失敗は、失敗した変更として記録しなければならない（MUST）。

#### Scenario: Failed change recorded
- Given: 変更`change-A`のapplyがエラーで終了した
- When: グループの実行が完了する
- Then: `change-A`は失敗した変更として記録される

#### Scenario: Failed change persists across groups
- Given: グループ1で`change-A`が失敗として記録された
- When: グループ2の実行が開始される
- Then: `change-A`は引き続き失敗した変更として追跡される

#### Scenario: Permission auto-reject is recorded as failed
- **GIVEN** apply出力に`permission requested`と`auto-rejecting`が含まれる
- **WHEN** changeがstalled/blockedとして扱われる
- **THEN** changeは失敗した変更として記録される
- **AND** 依存するchangeはスキップ判定の対象となる

### Requirement: Dependent Change Skipping

失敗した変更に依存する変更は、自動的にスキップされなければならない（MUST）。

さらに、`MergeWait` により未統合の change を依存先に持つ変更は実行を保留し、今回の run では実行してはならない（MUST）。依存未解決により実行できない change は queued 状態のまま保持され、ステータス表示は依存待ちであることを示さなければならない（MUST）。

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

#### Scenario: 依存待ち状態が表示される
- **GIVEN** 変更 A が base に未統合であり依存関係が未解決である
- **AND** 変更 B が変更 A に依存している
- **AND** 変更 B はキューに存在する
- **WHEN** 並列実行が次の実行対象を選定する
- **THEN** 変更 B は依存待ちとしてマークされる
- **AND** 変更 B のステータス表示は依存待ちであることを示す

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

### Requirement: Workspace Preservation on Error

並列実行においてエラーまたはユーザーによる強制停止が発生した場合、workspaceを削除せずに保持しなければならない（MUST）。また、成功マージが完了したworkspaceのみ削除してよい（MAY）。

#### Scenario: Workspace preserved on force stop
- **GIVEN** 並列実行が進行中である
- **AND** ユーザーがTUIで`Esc Esc`の強制停止を行う
- **WHEN** 並列実行がキャンセル扱いで早期終了する
- **THEN** worktreeは削除されず保持される
- **AND** 再開に利用できる状態が維持される

#### Scenario: Cleanup only after merged
- **GIVEN** 変更がマージ完了状態である
- **WHEN** クリーンアップが実行される
- **THEN** worktreeと対応ブランチが削除される
- **AND** マージ完了以外のworkspaceは削除されない

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

The system SHALL track the reason for loop termination (cancellation, graceful stop, normal completion, or merge_wait) using local state flags.

The system SHALL use this information to conditionally send completion events and messages.

加えて、`merge_wait` が残っている場合でも実行可能な change の処理が完了したときは `OrchestratorEvent::AllCompleted` を送信し、オーケストレーションは完了状態に遷移しなければならない（MUST）。

ただし、成功完了メッセージは `merge_wait` の有無を誤解させないように設計しなければならない（SHALL）。

#### Scenario: マージ待ちが残る場合でも完了イベントを送信する
- **GIVEN** 並列実行で少なくとも 1 件の change が `MergeWait` で残っている
- **AND** 実行可能な queued change の処理がすべて完了している
- **WHEN** 並列実行ループが終了処理に入る
- **THEN** システムは `OrchestratorEvent::AllCompleted` を送信する
- **AND** オーケストレーションは完了状態に遷移する

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
order-based再分析ループでもarchive完了後のmerge結果に応じてイベントを送信し、merge成功時にはcleanupイベントを送信しなければならない（SHALL）。
MergeDeferred が発生した場合は `MergeDeferred` イベントを送信し、待ち状態の表示は TUI 仕様に従って `MergeWait` または `ResolveWait` を判定しなければならない（SHALL）。

#### Scenario: order-based実行でmerge成功時にcleanupイベントを送信する
- **GIVEN** order-based再分析ループで変更Aのarchiveが完了している
- **WHEN** mergeが成功する
- **THEN** `CleanupStarted` と `CleanupCompleted` が送信される
- **AND** worktreeが削除される

#### Scenario: MergeDeferred はイベントとして送信される
- **GIVEN** order-based再分析ループで変更Aのarchiveが完了している
- **WHEN** mergeが `MergeDeferred` となる
- **THEN** `MergeDeferred` イベントが送信される

### Requirement: 並列モードのコミット起点対象判定
並列モードは、`HEAD` のコミットツリーに存在し、かつ `openspec/changes/<change_id>/` 配下に未コミットまたは未追跡ファイルが存在しない change だけを実行対象として扱わなければならない（SHALL）。

並列実行の開始時、システムはコミットツリーから `openspec/changes/<change-id>/` を列挙し、対象外の change を除外しなければならない（SHALL）。

#### Scenario: 未コミット change を除外する
- **GIVEN** `HEAD` のコミットツリーに存在しない change がある
- **WHEN** 並列実行が開始される
- **THEN** その change は実行対象から除外される
- **AND** 除外された change ID が警告ログに記録される

#### Scenario: change 配下の未コミット差分がある場合は除外する
- **GIVEN** `HEAD` のコミットツリーに存在する change がある
- **AND** `openspec/changes/<change_id>/` 配下に未コミットまたは未追跡ファイルが存在する
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
並列実行モードにおいて、order-based再分析ループでもarchive完了後に個別mergeを実行しなければならない（SHALL）。

merge開始前に `is_archive_commit_complete` を使用してworktreeのarchive完了状態を検証しなければならない（MUST）。検証条件は以下の通り:
1. worktreeがclean（未コミットの変更がない）
2. `openspec/changes/<change_id>` が存在しない
3. archiveエントリ（`openspec/changes/archive/<date>-<change_id>` または `openspec/changes/archive/<change_id>`）が存在する

上記いずれかの条件を満たさない場合は `MergeDeferred` を返し、`MergeWait` に留めなければならない（MUST）。

#### Scenario: order-based実行でarchive後にMergeDeferredとなる（changesが残っている）
- **GIVEN** order-based再分析ループで変更Aのarchiveコマンドが完了している
- **AND** `openspec/changes/{change_id}` が存在している
- **WHEN** `attempt_merge()` がmerge前の検証を行う
- **THEN** `is_archive_commit_complete` は `false` を返す
- **AND** `attempt_merge()` は `MergeDeferred` を返す
- **AND** 変更Aは `MergeWait` に留まる

#### Scenario: worktreeがdirtyの場合はMergeDeferred
- **GIVEN** order-based再分析ループで変更Aのarchiveコマンドが完了している
- **AND** worktreeがdirty（未コミットの変更がある）
- **WHEN** `attempt_merge()` がmerge前の検証を行う
- **THEN** `is_archive_commit_complete` は `false` を返す
- **AND** `attempt_merge()` は `MergeDeferred` を返す
- **AND** 失敗理由に「archive未完了」の文脈が含まれる

#### Scenario: archiveエントリが存在しない場合はMergeDeferred
- **GIVEN** order-based再分析ループで変更Aのarchiveコマンドが完了している
- **AND** `openspec/changes/{change_id}` は存在しない
- **AND** archiveエントリも存在しない
- **WHEN** `attempt_merge()` がmerge前の検証を行う
- **THEN** `is_archive_commit_complete` は `false` を返す
- **AND** `attempt_merge()` は `MergeDeferred` を返す

#### Scenario: archive完了状態でmergeが実行される
- **GIVEN** worktreeがclean
- **AND** `openspec/changes/{change_id}` が存在しない
- **AND** archiveエントリが存在する
- **WHEN** `attempt_merge()` がmerge前の検証を行う
- **THEN** `is_archive_commit_complete` は `true` を返す
- **AND** mergeが実行される

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

実行スロットの空き数は「アクティブな change の数」を基準に計算しなければならない（MUST）。アクティブな change は apply / acceptance / archive / resolve が進行中の change とし、merged / merge_wait / error / not queued はアクティブとして扱ってはならない（MUST NOT）。

依存関係は実行制約として扱い、`order` の上位にあっても依存先が base に Git マージされた状態（依存先の成果物を使って実行できる状態）になるまで開始してはならない（MUST）。

依存制約が解決した change は、依存解決後の実行開始時点で worktree を新規作成し、既存の worktree がある場合も作り直さなければならない（MUST）。この挙動は依存 change に固有であり、resume が常に成立することを保証しない前提の例外とする。

#### Scenario: 実行中の空きスロットでキュー追加が起動する
- **GIVEN** `max_concurrent_workspaces` が 3 に設定されている
- **AND** 進行中（apply / acceptance / archive / resolve）の change が 2 件である
- **AND** 実行中にキューへ新しい change が追加される
- **AND** 追加された change の依存関係はすべて解決済みである
- **WHEN** 実行スロットが空いたタイミングを迎える
- **THEN** システムはバッチ完了を待たずに新しい change を起動する
- **AND** 起動は `order` に従い空きスロット数を超えない

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

### Requirement: worktreeのtasks進捗読み取りは取得失敗で上書きしない
worktreeのtasks.mdから進捗を取得できない場合、archive/resolving中の進捗を0/0で上書きしてはならない（MUST NOT）。取得できる場合のみ進捗を更新しなければならない（MUST）。

#### Scenario: worktreeのtasks取得失敗時は進捗を維持する
- **GIVEN** worktreeが存在し、変更がArchivingまたはResolving状態である
- **AND** 直前のprogressが0/0ではない
- **WHEN** worktree/archived fallbackのtasks.md読み取りが失敗する
- **THEN** 進捗は直前の値を維持する

### Requirement: スロット駆動の連続ディスパッチ

並列実行はバッチ/グループ完了を待たず、実行スロットが空いたタイミングごとに依存関係分析の `order` に従って次の変更を選定しなければならない（MUST）。

#### Scenario: スロットが空いたら即時に次の変更を選定する
- **GIVEN** `max_concurrent_workspaces` が 3 に設定されている
- **AND** 進行中の change が 2 件である
- **AND** キューに未実行の change が存在する
- **WHEN** 実行スロットが空く
- **THEN** システムはバッチ完了を待たずに次の change を選定する

### Requirement: Re-analysis triggers and non-blocking scheduler
re-analysis は apply/acceptance/archive/resolve の in-flight が存在していても開始できなければならない（MUST）。

re-analysis ループは dispatch の完了待ちでブロックされてはならない（MUST NOT）。

re-analysis の起動トリガは、キュー通知・デバウンスタイマー・in-flight 完了のいずれでもよい（MUST）。

利用可能スロットが 0 の場合、システムは re-analysis を実行せず、空きができた時点で re-analysis を再評価しなければならない（MUST）。

スケジューラの実装は、再分析・ディスパッチ選定・完了処理の責務をヘルパー関数に分割してもよい（MAY）。ただし、非ブロッキング性と起動条件の挙動は維持しなければならない（MUST）。

#### Scenario: キュー変化でre-analysisが起動する
- **GIVEN** apply 実行中の change が存在する
- **AND** queued に新しい change が追加される
- **WHEN** 並列実行が re-analysis を評価する
- **THEN** apply 完了を待たずに re-analysis が開始される

#### Scenario: in-flight 完了でre-analysisが再開する
- **GIVEN** apply/acceptance/archive/resolve の in-flight が存在する
- **AND** queued に別の change が存在する
- **WHEN** in-flight の change が完了する
- **THEN** re-analysis が再評価される

#### Scenario: dispatch が re-analysis ループをブロックしない
- **GIVEN** in-flight の change が存在する
- **AND** queued に別の change が存在する
- **WHEN** 並列実行が dispatch を開始する
- **THEN** re-analysis ループは apply 完了を待たずに次のトリガ待ちへ戻る

#### Scenario: スロットが空いていない場合はre-analysisしない
- **GIVEN** 利用可能なスロットが0である
- **AND** queued に change が存在する
- **WHEN** 並列実行が re-analysis を評価する
- **THEN** re-analysis は実行されない
- **AND** スロットが空いた時点で re-analysis が再評価される

### Requirement: In-flight tracking and slot-based dispatch

システムは in-flight の change を追跡し、空きスロット数を算出しなければならない（MUST）。

in-flight は apply/acceptance/archive/resolve の change とし、resolve には並列実行による自動 resolve と TUI からの手動 resolve の両方を含めなければならない（MUST）。merged/merge_wait/error/not queued を in-flight として扱ってはならない（MUST NOT）。

空きスロット数は `max_concurrent_workspaces - in_flight_count` で算出し、0 未満にならないように扱わなければならない（MUST）。

re-analysis の `order` は依存関係の制約として扱い、依存解決済みの change だけを空きスロット数分 dispatch しなければならない（MUST）。

#### Scenario: 空きスロット数に応じてdispatchする
- **GIVEN** `max_concurrent_workspaces` が 3 である
- **AND** in-flight が 2 件である
- **AND** queued に依存解決済みの change が 2 件ある
- **WHEN** re-analysis が dispatch を行う
- **THEN** 1 件のみ dispatch される

#### Scenario: in-flight に非アクティブ状態が含まれない
- **GIVEN** merged/merge_wait/error/not queued の change が存在する
- **WHEN** 並列実行が in-flight を算出する
- **THEN** それらの change は in-flight として数えられない

#### Scenario: 手動 resolve は in-flight に含まれる
- **GIVEN** `max_concurrent_workspaces` が 3 である
- **AND** apply/acceptance/archive で in-flight が 2 件である
- **AND** TUI から手動 resolve が開始される
- **WHEN** 並列実行が空きスロット数を算出する
- **THEN** in-flight は 3 件として扱われる
- **AND** queued の change はスロットが空くまで dispatch されない

### Requirement: Queue ingestion and analysis targeting
並列実行の analysis は queued の change のみを対象にしなければならない（MUST）。

キューに追加された change は analysis 実行前に queued 集合へ反映されなければならない（MUST）。

queued の change が空の場合、analysis を実行してはならない（MUST）。

実行中の change が存在せず、queued の change も空の場合、オーケストレーションは完了状態にならなければならない（MUST）。

queued に含まれない change（例: merged 済み change、実行済み change、削除済み change）は analysis 対象から除外されなければならない（MUST）。

#### Scenario: queuedのみがanalysis対象になる
- **GIVEN** queued に change が存在する
- **AND** queued 以外に実行中の change が存在する
- **WHEN** 並列実行が analysis を開始する
- **THEN** analysis 対象は queued の change のみになる

#### Scenario: queuedが空ならanalysisを実行しない
- **GIVEN** queued の change が存在しない
- **WHEN** 並列実行が analysis を開始しようとする
- **THEN** analysis を実行しない

#### Scenario: 実行中とqueuedが空なら終了する
- **GIVEN** 実行中の change が存在しない
- **AND** queued の change も空である
- **WHEN** 並列実行ループが次の analysis を開始しようとする
- **THEN** analysis を実行しない
- **AND** オーケストレーションは完了状態になる

#### Scenario: queued外のchangeはanalysis対象から除外される
- **GIVEN** queued に含まれない change が存在する
- **AND** queued には別の change が存在する
- **WHEN** 並列実行が analysis を開始する
- **THEN** queued 外の change は analysis 対象から除外される

### Requirement: Dispatch sequencing for queued changes
キューに追加された change は analysis を経由せずに dispatch されてはならない（MUST NOT）。

dispatch は re-analysis ループのスケジューラによってのみ起動され、apply 側の補助ロジックから直接 spawn されてはならない（MUST）。

#### Scenario: 追加されたchangeはanalysis経由でdispatchされる
- **GIVEN** queued に新しい change が追加される
- **WHEN** 並列実行が次の dispatch を開始する
- **THEN** change は analysis の `order` に含まれている
- **AND** dispatch はスケジューラ経由でのみ起動される

### Requirement: In-flight Change Cancellation
並列実行中にTUIから単体停止が要求された場合、対象changeの実行はキャンセルされなければならない（SHALL）。キャンセル完了後、当該changeは in-flight から除外され、queued が残っている場合は再分析が実行されなければならない（SHALL）。

#### Scenario: Cancel active change and re-analyze remaining queued
- **GIVEN** parallel execution is running with multiple queued changes
- **AND** one change is in-flight
- **WHEN** a stop request for the in-flight change is issued
- **THEN** the in-flight change is cancelled and removed from in-flight tracking
- **AND** analysis runs for remaining queued changes
- **AND** the remaining queued changes continue execution

### Requirement: Permission Auto-Reject Handling
apply実行中にエージェント出力から権限要求のauto-rejectが検出された場合、システムは当該changeを実行不能として扱わなければならない（MUST）。

システムは以下を満たさなければならない（MUST）。
- applyの再試行を停止する
- stalled/blockedとして記録する
- 理由に拒否されたパスと権限設定の案内を含める
- 空WIPコミットによるstall検出を当該changeについては実行しない
- 依存スキップの判定に反映する

#### Scenario: Permission auto-reject is detected during apply
- **GIVEN** apply出力に`permission requested`と`auto-rejecting`が含まれる
- **WHEN** applyループが出力を評価する
- **THEN** changeはstalled/blockedとして記録される
- **AND** applyの再試行は行われない
- **AND** stall検出（空WIPコミット）は実行されない
- **AND** 理由に拒否パスと権限設定の案内が含まれる

#### Scenario: Non-permission errors do not trigger permission handling
- **GIVEN** apply出力にpermission auto-rejectが含まれない
- **WHEN** applyループが出力を評価する
- **THEN** 通常の失敗処理が適用される


### Requirement: Resumed Archived Workspaces Preserve Merge Handoff

When parallel execution resumes a workspace already detected as `WorkspaceState::Archived`, the executor SHALL treat that workspace as archive-complete for downstream lifecycle handling.

The resumed workspace MUST NOT silently complete in a way that bypasses merge handling or causes the change to regress to `NotQueued` before merge resolution is attempted.

#### Scenario: Resumed archived workspace enters merge wait on restart

- **GIVEN** a parallel worktree is reused on restart
- **AND** `detect_workspace_state(change_id, workspace_path, base_branch)` returns `WorkspaceState::Archived`
- **AND** the change is not yet merged into the base branch
- **WHEN** the resumed change is dispatched
- **THEN** apply and archive are not re-run
- **AND** the resumed change is handed off to the same archive-complete lifecycle used by a freshly archived change
- **AND** the change transitions to merge handling or `MergeWait`, not `NotQueued`

#### Scenario: Resumed archived workspace participates in merge-deferred flow

- **GIVEN** a reused worktree is already `WorkspaceState::Archived`
- **AND** merge cannot proceed immediately
- **WHEN** the resumed change completes dispatch/completion handling
- **THEN** the system emits the same archive-complete semantics used by normal archive success
- **AND** merge handling returns `MergeDeferred`
- **AND** the change remains in `MergeWait`

#### Scenario: Mixed archiving restart does not drop archived change from queue lifecycle

- **GIVEN** three parallel workspaces are reused after an interrupted run
- **AND** two workspaces are still `WorkspaceState::Archiving`
- **AND** one workspace is already `WorkspaceState::Archived`
- **WHEN** the restarted parallel run resumes those workspaces
- **THEN** all three changes converge to archive-complete merge handling as their resume paths finish
- **AND** none of the resumed changes regresses to `NotQueued` solely because archive completed before shutdown


#


### Requirement: Parallel Execution Event Reporting

order-based再分析ループでもarchive完了後のmerge結果に応じてイベントを送信し、merge成功時にはcleanupイベントを送信しなければならない（SHALL）。
MergeDeferred が発生した場合は `MergeDeferred` イベントを送信し、待機状態の表示は TUI 仕様に従って `MergeWait` または `ResolveWait` を判定しなければならない（SHALL）。

さらに、`MergeDeferred` のうち先行 merge / resolve の完了で再評価可能な change は、自動再評価対象として保持されなければならない（MUST）。
先行 merge または resolve が完了したとき、システムは自動再評価対象の change を再評価し、競合が残る場合は `ResolveWait` または `Resolving` に進め、merge 再試行可能な場合は `MergeWait` に留めてはならない（MUST）。
手動介入が必要な change のみが `MergeWait` に留まらなければならない（MUST）。

#### Scenario: 先行 merge 完了後に deferred change が自動再評価される
- **GIVEN** change B が `MergeDeferred` となっている
- **AND** その理由は先行している change A の merge / resolve 完了待ちである
- **WHEN** change A の merge または resolve が完了する
- **THEN** システムは change B を自動再評価する
- **AND** change B は `MergeWait` のまま放置されない

#### Scenario: 自動再評価後に競合が残る change は resolve 待機へ進む
- **GIVEN** change B が先行 merge 完了待ちの `MergeDeferred` として保持されている
- **WHEN** 再評価時点でも change B に解消すべき競合が残っている
- **THEN** change B は `ResolveWait` または `Resolving` に進む
- **AND** 手動 `M` を押さなくても次の解決フローに乗る

#### Scenario: 手動介入が必要な deferred change だけが MergeWait に残る
- **GIVEN** change B が `MergeDeferred` となっている
- **AND** システムが競合原因を再評価しても自動再開条件を満たさない
- **WHEN** 待機状態が更新される
- **THEN** change B は `MergeWait` に留まる
- **AND** TUI は手動 resolve 対象として表示する


### Requirement: Scheduler Loop Termination

The scheduler loop SHALL NOT terminate while any change is in ResolveWait state (auto-resumable merge deferred) or while a manual resolve is actively running.

The scheduler loop SHALL terminate when all of the following conditions are met:
- `queued` changes list is empty
- `in_flight` changes set is empty
- `resolve_wait_changes` set is empty (no auto-resumable deferred merges pending)
- Manual resolve counter is zero (no resolve commands actively executing)
- `join_set` is empty (no spawned tasks running)

Changes in MergeWait state (requiring user intervention) SHALL NOT prevent scheduler loop termination.

#### Scenario: ResolveWait prevents scheduler exit

**Given**: All apply/archive tasks have completed
**And**: One change is in ResolveWait state (auto_resumable merge deferred)
**And**: The queued list and in_flight set are empty
**When**: The scheduler loop evaluates its break conditions
**Then**: The scheduler loop SHALL continue running
**And**: Dynamic queue notifications SHALL be processed (new changes can be analyzed and dispatched)

#### Scenario: MergeWait does not prevent scheduler exit

**Given**: All apply/archive tasks have completed
**And**: One change is in MergeWait state (requires user intervention)
**And**: No changes are in ResolveWait state
**And**: Manual resolve counter is zero
**When**: The scheduler loop evaluates its break conditions
**Then**: The scheduler loop SHALL terminate and send AllCompleted

#### Scenario: Queue addition during ResolveWait triggers analysis

**Given**: The scheduler loop is running with one change in ResolveWait
**And**: Run slots are available (in_flight + resolve count < max_parallelism)
**When**: A new change is added to the dynamic queue
**Then**: The scheduler SHALL analyze and dispatch the new change

### Requirement: Merge Deferred State Separation

The parallel executor SHALL maintain two separate sets for tracking deferred merge changes:

- `resolve_wait_changes`: Changes with auto-resumable deferral reasons (e.g., another merge in progress). These are considered "in progress" and keep the scheduler alive.
- `merge_wait_changes`: Changes requiring user intervention (e.g., uncommitted changes on base). These are considered "suspended" and do not keep the scheduler alive.

When a `MergeAttempt::Deferred` result is received, the change SHALL be added to `resolve_wait_changes` if `auto_resumable` is true, or `merge_wait_changes` if `auto_resumable` is false.

The `retry_deferred_merges` method SHALL only retry changes in `resolve_wait_changes`. If a retry results in a non-auto-resumable deferral, the change SHALL be moved from `resolve_wait_changes` to `merge_wait_changes`.

#### Scenario: Auto-resumable deferral tracked as resolve_wait

**Given**: A change completes apply and archive successfully
**When**: The merge attempt returns `Deferred` with `auto_resumable=true`
**Then**: The change is added to `resolve_wait_changes`
**And**: The scheduler loop does not terminate

#### Scenario: Manual-intervention deferral tracked as merge_wait

**Given**: A change completes apply and archive successfully
**When**: The merge attempt returns `Deferred` with `auto_resumable=false`
**Then**: The change is added to `merge_wait_changes`
**And**: The scheduler loop may terminate if no other active work remains


### Requirement: Parallel execution acceptance loop
Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace.
The acceptance loop SHALL parse stdout to determine pass/fail/continue/blocked, and MUST NOT use exit code to determine acceptance verdict.
The acceptance prompt MUST include a hardcoded acceptance prompt followed by configured `acceptance_prompt`.
When resuming a workspace that has not completed archive, the orchestrator SHALL re-run acceptance before starting archive, even if tasks are already complete.

**Acceptance state persistence**: Acceptance results are NOT persisted to disk or git commits. Therefore, on resume:
- If workspace state is `Applying` or `Created`: Normal apply+acceptance loop proceeds
- If workspace state is `Applied`: Acceptance MUST be re-run before archive
- If workspace state is `Archiving` (archive files moved but not committed): Acceptance MUST be re-run before archive commit
- If workspace state is `Archived` or `Merged`: Acceptance is not required (archive already complete)

This ensures quality gates are always enforced, even after interruptions.

- The second and later acceptance attempts MUST focus on the updated file list since the previous acceptance attempt and the previously reported findings, rather than performing a full re-check.
- The acceptance prompt for second and later attempts MUST include the updated file list (file paths only) since the previous acceptance attempt.
- The acceptance prompt for second and later attempts MUST include the previous acceptance findings and instruct the agent to verify whether those findings are resolved.
- The acceptance prompt for second and later attempts MUST instruct the agent to read relevant files as needed; it MUST NOT include diff content.
- Acceptance failures SHALL record findings using stdout/stderr tail lines without parsing `FINDINGS:` structure.
- Acceptance findings MUST exclude `ACCEPTANCE:` markers and the `FINDINGS:` header line from the recorded tail lines.
- Acceptance FAIL logs MUST NOT label tail line counts as "findings"; if counts are shown, they MUST be labeled as tail lines.
- If acceptance output is BLOCKED, the orchestrator MUST stop apply retries for the change and preserve the workspace for manual follow-up.
- If acceptance output is BLOCKED, the change MUST be recorded as a terminal failure for dependency skipping in the current run.

**Archive commit creation**: When `ensure_archive_commit()` detects a dirty working tree after the archive command, it SHALL first attempt a direct `git add -A && git commit` with message `"Archive: {change_id}"` without invoking the AI resolve command. If the direct commit succeeds and `is_archive_commit_complete()` returns true, the archive commit is considered finalized. If the direct commit fails (e.g., due to pre-commit hooks modifying files or rejecting the commit), the system SHALL fall back to the AI resolve command for recovery.

#### Scenario: Parallel acceptance retry narrows to updated files and prior findings
- **GIVEN** a change completes an apply iteration successfully in parallel mode
- **AND** acceptance output indicates CONTINUE
- **WHEN** the orchestrator runs a subsequent acceptance attempt for the same change
- **THEN** the acceptance prompt includes only the updated file list since the previous acceptance attempt (no diff content)
- **AND** the acceptance prompt includes the prior acceptance findings for verification
- **AND** the acceptance prompt instructs the agent to read files as needed to confirm fixes

#### Scenario: Parallel acceptance failure logging uses tail lines
- **GIVEN** acceptance output tail includes `ACCEPTANCE: FAIL` and `FINDINGS:` lines
- **WHEN** the orchestrator records the acceptance failure
- **THEN** the recorded findings exclude the acceptance markers and `FINDINGS:` header
- **AND** logs do not report "N findings" based on tail line count

#### Scenario: Acceptance blocked preserves workspace and stops apply
- **GIVEN** acceptance output indicates `ACCEPTANCE: BLOCKED`
- **WHEN** the orchestrator processes the acceptance result
- **THEN** the workspace is preserved for manual follow-up
- **AND** apply retries for the change are stopped in the current run

#### Scenario: Direct archive commit succeeds without AI resolve
- **GIVEN** a change has been archived (files moved to archive directory)
- **AND** the working tree is dirty with uncommitted archive changes
- **WHEN** `ensure_archive_commit()` is called
- **THEN** the system executes `git add -A && git commit -m "Archive: {change_id}"` directly
- **AND** the AI resolve command is NOT invoked
- **AND** `is_archive_commit_complete()` returns true

#### Scenario: Direct archive commit fails and falls back to AI resolve
- **GIVEN** a change has been archived (files moved to archive directory)
- **AND** the working tree is dirty with uncommitted archive changes
- **AND** a pre-commit hook rejects or modifies the commit
- **WHEN** `ensure_archive_commit()` attempts the direct commit
- **AND** the direct commit fails (non-zero exit code)
- **THEN** the system logs a warning about the direct commit failure
- **AND** the system falls back to the AI resolve command
- **AND** the AI resolve command attempts to finalize the archive commit


### Requirement: Shared Parallel Orchestration Service

システムはCLIとTUIの並列実行を扱う統一的な`ParallelRunService`を提供しなければならない（SHALL）。

サービスはイベント通知のためのコールバック機構を受け取り、TUIへ送るイベントは共有状態の更新より先に送信しなければならない（MUST）。これによりUI更新が共有状態のロック待ちで遅延しない。

サービスは以下をカプセル化すること：
- Git availability checking
- Change grouping by dependencies
- ParallelExecutor coordination
- Archiving of completed changes
- Rejection of blocked changes (acceptance Blocked → rejection flow)

ParallelRunService は、コミットツリーに存在しない change の除外と警告通知を CLI/TUI のどちらの経路でも同一ロジックで実行しなければならない（SHALL）。

Acceptance が `Blocked` を返した場合、ParallelRunService は rejection フロー（`REJECTED.md` 生成 → `REJECTED.md` のみを base にコミット → worktree 削除）を実行し、`WorkspaceResult` で `error: None, rejected: Some(reason)` を返さなければならない（SHALL）。

#### Scenario: CLI uses ParallelRunService

- **WHEN** the CLI runs in parallel mode (`--parallel` flag)
- **THEN** the CLI SHALL use `ParallelRunService` to execute changes
- **AND** events SHALL be logged to stdout via the callback mechanism

#### Scenario: TUI uses ParallelRunService

- **WHEN** the TUI runs in parallel mode
- **THEN** the TUI SHALL use `ParallelRunService` to execute changes
- **AND** events SHALL be forwarded to the TUI event channel via the callback mechanism
- **AND** event forwarding happens before shared state updates so Accepting can render promptly

#### Scenario: Acceptance Blocked triggers rejection flow in parallel mode

- **GIVEN** a change is executing in parallel mode
- **WHEN** acceptance returns `Blocked`
- **THEN** the rejection flow SHALL execute within the worktree context
- **AND** the worktree SHALL be deleted after rejection completes
- **AND** `WorkspaceResult.rejected` SHALL contain the rejection reason
- **AND** `WorkspaceResult.error` SHALL be `None`


### Requirement: Non-blocking Merge in Scheduler Loop

パラレルスケジューラの `tokio::select!` イベントループは、workspace 完了後の merge + コンフリクト解決処理によってブロックされてはならない（MUST NOT）。merge + resolve 処理はバックグラウンドタスクとして非同期に実行し、スケジューラループは queued change の dispatch を継続しなければならない（SHALL）。

merge/resolve の結果（成功・Deferred・失敗）はスケジューラループに非同期に通知され、適切に処理されなければならない（MUST）。

#### Scenario: Queued change dispatched during resolve

- **GIVEN** Change A のコンフリクト解決（resolve）が進行中で、queued に Change B が存在し、利用可能スロットが 1 以上ある
- **WHEN** スケジューラループの次の iteration が実行される
- **THEN** Change B の re-analysis と dispatch が実行される
- **AND** Change A の resolve は並行して継続する

#### Scenario: Merge result delivered after background completion

- **GIVEN** Change A の merge がバックグラウンドタスクで実行中
- **WHEN** merge が成功する
- **THEN** merge 結果がスケジューラループに通知される
- **AND** `retry_deferred_merges` が呼び出され、ResolveWait の change がリトライされる

#### Scenario: Merge deferred delivered after background attempt

- **GIVEN** Change A の merge がバックグラウンドで試行される
- **WHEN** merge が Deferred（resolve 進行中 or base dirty）となる
- **THEN** Deferred イベントがスケジューラループに通知される
- **AND** Change A は resolve_wait_changes または merge_wait_changes に追加される


### Requirement: Parallel Execution Event Reporting

order-based再分析ループでもarchive完了後のmerge結果に応じてイベントを送信し、merge成功時にはcleanupイベントを送信しなければならない（SHALL）。

MergeDeferred が発生した場合は `MergeDeferred` イベントを送信し、待ち状態の表示は TUI 仕様に従って `MergeWait` または `ResolveWait` を判定しなければならない（SHALL）。

さらに、`MergeDeferred` のうち先行 merge / resolve の完了で再評価可能な change は、自動再評価対象として保持されなければならない（MUST）。
先行 merge または resolve が完了したとき、システムは自動再評価対象の change を再評価し、競合が残る場合は `ResolveWait` または `Resolving` に進め、merge 再試行可能な場合は `MergeWait` に留めてはならない（MUST）。
手動介入が必要な change のみが `MergeWait` に留まらなければならない（MUST）。

Git backend では archive-complete 後の merge/dependency 判定に先立って base branch (`original_branch`) を初期化しなければならない（MUST）。初期化可能な場合、システムは self-heal して merge handling を継続し、`Original branch not initialized` を理由に archived change を `MergeWait` に留めてはならない（MUST）。recover 不能な detached HEAD 等のみが実行エラーとして報告されてよい（MAY）。

#### Scenario: archived merge self-heals when base branch was not yet initialized
- **GIVEN** a parallel Git worktree has already completed archive
- **AND** the archived change is being handed off into merge handling
- **AND** the workspace manager has not yet cached `original_branch`
- **WHEN** merge handling starts
- **THEN** the system initializes the base branch from the repository state before merge evaluation
- **AND** merge handling continues without surfacing `Original branch not initialized`
- **AND** the change does not remain in `MergeWait` solely due to the missing initialization

#### Scenario: unrecoverable base branch discovery fails as execution error
- **GIVEN** a parallel Git worktree has already completed archive
- **AND** merge handling cannot determine a base branch because the repository is in detached HEAD state
- **WHEN** merge handling starts
- **THEN** the system reports an execution error rather than classifying the change as manual-intervention `MergeWait`
- **AND** the failure is distinguishable from deferred merge conflicts or base-dirty waits


### Requirement: Parallel execution completion status must accurately reflect actual processing outcome

The system SHALL send completion events and messages only when processing completes normally, not when stopped or cancelled by the user.

The system SHALL distinguish between successful completion, completion with errors, graceful stop, and cancellation.

The parallel execution subsystem SHALL NOT run a merge stall monitor based on historical base-branch merge commit timestamps. Queue execution MUST NOT be interrupted or annotated by a monitor that does not observe current queue or scheduler progress.

**Priority**: HIGH
**Rationale**: Incorrect completion messages mislead users about the processing status and can cause confusion when resuming work. A monitor that watches unrelated historical merge activity does not represent actual queue health and should not participate in parallel execution.

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

#### Scenario: Parallel execution does not start merge stall monitoring

**Given** the orchestrator is running in parallel mode
**When** the parallel execution loop starts
**Then** it does not start a merge stall monitor based on historical base-branch merge commits
**And** queue execution proceeds based only on actual execution state, user stop requests, and real processing failures


### Requirement: ParallelRunService rejection flow on blocked execution

ParallelRunService SHALL support blocked handoff from both acceptance and apply execution phases. When apply execution records a blocker by generating `openspec/changes/<change_id>/REJECTED.md` as a rejection proposal, the runtime SHALL treat the workspace as `apply blocked` even if `tasks.md` still contains unchecked items. An `apply blocked` workspace SHALL proceed to acceptance instead of being retried indefinitely as fresh apply work. Acceptance SHALL decide whether to confirm the rejection proposal, and only a confirmed blocked verdict SHALL execute the rejection flow.

#### Scenario: apply blocker proposal reaches acceptance

- **GIVEN** apply execution generates `openspec/changes/fix-auth/REJECTED.md` with a blocker reason
- **AND** `openspec/changes/fix-auth/tasks.md` still contains unchecked implementation tasks
- **WHEN** the runtime evaluates the apply result
- **THEN** the workspace is treated as `apply blocked`
- **AND** the change proceeds to acceptance instead of looping in apply retries

#### Scenario: confirmed blocked verdict runs rejection flow

- **GIVEN** acceptance receives a change in `apply blocked` state with a rejection proposal
- **WHEN** acceptance confirms the blocked verdict
- **THEN** the rejection flow executes
- **AND** the worktree is cleaned up after rejection completes

#### Scenario: unconfirmed blocker does not trigger rejection flow

- **GIVEN** acceptance receives a change in `apply blocked` state with a rejection proposal
- **WHEN** acceptance does not confirm rejection
- **THEN** the rejection flow does not execute
- **AND** the runtime returns the change to a non-terminal state for further action


### Requirement: ParallelRunService rejection flow on blocked execution

ParallelRunService SHALL treat a confirmed blocked verdict as a terminal rejection after the base branch has recorded `openspec/changes/<change_id>/REJECTED.md`. Parallel rejection handling SHALL NOT rely on `openspec resolve <change_id>` and SHALL NOT merge additional worktree files into the base branch. After the reject marker commit succeeds, the runtime SHALL emit a rejected result, preserve the rejection reason, and clean up the rejected worktree.

#### Scenario: parallel rejected result is driven by REJECTED marker commit

- **GIVEN** acceptance confirms a blocked verdict in parallel mode
- **WHEN** the rejection flow commits `openspec/changes/fix-auth/REJECTED.md` on the base branch
- **THEN** the workspace result is returned as rejected
- **AND** no further resolve step is required to finalize the rejection

#### Scenario: rejected worktree changes are not merged to base

- **GIVEN** a rejected worktree contains code, tasks, and spec changes in addition to `REJECTED.md`
- **WHEN** the rejection flow completes
- **THEN** the base branch receives only `openspec/changes/fix-auth/REJECTED.md`
- **AND** the remaining worktree-only files are discarded with worktree cleanup


### Requirement: Parallel execution acceptance loop

Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace.
The acceptance loop SHALL parse stdout to determine pass/fail/continue/blocked, and MUST NOT use exit code to determine acceptance verdict.
The acceptance prompt MUST include a hardcoded acceptance prompt followed by configured `acceptance_prompt`.
When resuming a workspace that has not completed archive, the orchestrator SHALL re-run acceptance before starting archive, even if tasks are already complete.
The second and later acceptance attempts MUST focus on the updated file list since the previous acceptance attempt and the previously reported findings, rather than performing a full re-check.
The acceptance prompt for second and later attempts MUST include the updated file list (file paths only) since the previous acceptance attempt.
The acceptance prompt for second and later attempts MUST include the previous acceptance findings and instruct the agent to verify whether those findings are resolved.
The acceptance prompt for second and later attempts MUST instruct the agent to read relevant files as needed; it MUST NOT include diff content.
Acceptance findings MUST exclude `ACCEPTANCE:` markers and the `FINDINGS:` header line from the recorded tail lines.
If acceptance output is BLOCKED, the orchestrator MUST stop apply retries for the change and preserve the workspace for manual follow-up.
If acceptance output is BLOCKED, the change MUST be recorded as a terminal failure for dependency skipping in the current run.
Before allowing archive to start, acceptance MUST verify that the workspace is ready for a real final archive commit under the repository's final-commit quality gates (SHALL). If those readiness checks fail, acceptance MUST return a non-pass verdict and record the blocking gate or command context instead of allowing archive to surface the failure later (MUST).

#### Scenario: Parallel acceptance retry narrows to updated files and prior findings
- **GIVEN** a change completes an apply iteration successfully in parallel mode
- **AND** acceptance output indicates CONTINUE
- **WHEN** the orchestrator runs a subsequent acceptance attempt for the same change
- **THEN** the acceptance prompt includes only the updated file list since the previous acceptance attempt (no diff content)
- **AND** the acceptance prompt includes the prior acceptance findings for verification
- **AND** the acceptance prompt instructs the agent to read files as needed to confirm fixes

#### Scenario: Parallel acceptance failure logging uses tail lines
- **GIVEN** acceptance output tail includes `ACCEPTANCE: FAIL` and `FINDINGS:` lines
- **WHEN** the orchestrator records the acceptance failure
- **THEN** the recorded findings exclude the acceptance markers and `FINDINGS:` header
- **AND** logs do not report "N findings" based on tail line count

#### Scenario: Acceptance blocked preserves workspace and stops apply
- **GIVEN** acceptance output indicates `ACCEPTANCE: BLOCKED`
- **WHEN** the orchestrator processes the acceptance result
- **THEN** the workspace is preserved for manual follow-up
- **AND** apply retries for the change are stopped in the current run

#### Scenario: Acceptance catches archive-readiness blocker before archive
- **GIVEN** apply has produced a workspace that appears functionally complete
- **AND** the final archive commit would be rejected by a repository quality gate such as a pre-commit hook, formatting check, lint check, or test gate
- **WHEN** acceptance evaluates archive-readiness
- **THEN** acceptance returns a non-pass verdict before archive starts
- **AND** acceptance findings identify the blocking gate or command context so the failure is actionable

#### Scenario: Acceptance passes archive-ready workspace to archive
- **GIVEN** apply has produced a workspace with no unresolved acceptance findings
- **AND** the workspace satisfies the repository's final-commit quality gates for archive
- **WHEN** acceptance completes
- **THEN** the change may proceed to archive
- **AND** archive remains responsible for executing and verifying the final archive commit
