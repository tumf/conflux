# parallel-execution Specification

## Purpose
Defines parallel change execution using jj workspaces or Git worktrees.
## Requirements

### Requirement: Shared Parallel Orchestration Service

Parallel merge retry dispatch SHALL handle deferred merge attempts as pending/deferred outcomes rather than successful completed background merge tasks. A background merge task MUST report successful completion only when the merge attempt actually integrated the change into base or determined from repository-visible evidence that the change was already integrated.

Parallel ordinary apply dispatch MUST treat reducer terminal-error state as a stop gate. After a change emits an apply, acceptance, archive, dispatch, or workspace execution error, scheduler reanalysis, queue reconciliation, and workspace resume scans MUST NOT dispatch that same change to apply again unless explicit retry intent has cleared the recoverable error terminal state. Existing workspaces MAY remain available for operator inspection or explicit retry.

Dependency analysis MUST continue to treat an errored dependency as a dispatch blocker for dependents until the dependency is explicitly retried and reaches repository-visible success.

<!-- Expected canonical result after archive: `parallel-execution` will require terminal-error changes to stay stopped across reanalysis/resume until explicit retry clears the reducer error. -->

#### Scenario: parallel apply error is not automatically redispatched

**Given**: change `alpha` is running in parallel apply
**When**: the workspace task emits `ProcessingError` or `ApplyFailed` for `alpha`
**Then**: `alpha` is recorded as `error`
**And**: the next scheduler reanalysis does not select `alpha` for ordinary apply dispatch
**And**: `alpha` remains available for explicit retry rather than being removed silently

#### Scenario: workspace resume does not resurrect errored change

**Given**: change `alpha` has terminal state `Error`
**And**: an existing workspace for `alpha` remains on disk
**When**: parallel workspace resume or repair-candidate scanning runs
**Then**: `alpha` is not dispatched to ordinary apply solely because the workspace exists
**And**: `alpha` remains displayed as `error` until explicit retry or delayed repository-visible success

#### Scenario: explicit retry restores parallel dispatch eligibility

**Given**: change `alpha` has terminal state `Error`
**And**: the operator explicitly marks `alpha` for retry
**When**: the retry transition clears the recoverable error terminal state
**Then**: `alpha` may be selected by normal parallel dependency analysis and dispatch rules
**And**: unmarked error changes remain excluded from ordinary apply dispatch

#### Scenario: errored dependency blocks dependent dispatch

**Given**: queued change `beta` depends on change `alpha`
**And**: `alpha` has terminal state `Error`
**When**: parallel dependency analysis selects dispatch candidates
**Then**: `beta` is not dispatched
**And**: after `alpha` is explicitly retried and reaches repository-visible success, `beta` may be re-evaluated by normal dependency analysis

### Requirement: Archived dependency references are explicitly classified

The system SHALL classify active proposal metadata dependency targets using repository-visible evidence that distinguishes queued, in-flight, archived, rejected, and missing targets.

Archived dependency references MUST NOT collapse into generic parse/json failures. Rejected dependency references MUST NOT collapse into generic missing dependency failures when `REJECTED.md` evidence exists.

Rejected and missing dependency targets SHALL remain fail-closed dispatch blockers, while archived dependency targets SHALL be treated as satisfied.

#### Scenario: Archived dependency is surfaced with dedicated diagnostics

- **GIVEN** active change `alpha` declares dependency `beta`
- **AND** `beta` exists only under `openspec/changes/archive/`
- **WHEN** analyze or validate checks the dependency target
- **THEN** diagnostics classify the target as an archived dependency reference
- **AND** diagnostics are not displayed as generic `Analysis returned invalid JSON`

#### Scenario: Missing dependency remains an invalid dependency failure

- **GIVEN** active change `alpha` declares dependency `gamma`
- **AND** `gamma` is not queued, in-flight, archived, or rejected
- **WHEN** analyze or validate checks the dependency target
- **THEN** diagnostics classify the target as missing
- **AND** the message is distinguishable from archived and rejected dependency cases

#### Scenario: Rejected dependency remains a terminal dispatch blocker

- **GIVEN** active change `alpha` declares dependency `beta`
- **AND** `openspec/changes/beta/proposal.md` exists
- **AND** `openspec/changes/beta/REJECTED.md` exists
- **WHEN** analyze or scheduler dispatch checks the dependency target
- **THEN** diagnostics classify the target as rejected
- **AND** `alpha` is not dispatched
- **AND** the message is distinguishable from a missing dependency

### Requirement: Dependency-blocked diagnostics are stable and non-spamming

The scheduler SHALL preserve dependency-blocked state for queued changes that cannot dispatch, but it MUST NOT repeatedly emit identical operator-visible blocked/error diagnostics while the blocked change has the same repository-visible dependency blocker signature.

A blocker signature SHALL include at least the blocked change id, dependency ids, and dependency target classes. When the signature changes, the scheduler SHALL emit a fresh diagnostic and re-evaluate dispatch using the updated dependency evidence.

#### Scenario: Repeated rejected dependency blocker does not spam logs

- **GIVEN** queued change `alpha` depends on rejected dependency `beta`
- **AND** the scheduler has already emitted an operator-visible diagnostic for blocker signature `alpha -> beta [rejected]`
- **WHEN** later scheduler loops observe the same blocker signature
- **THEN** `alpha` remains dependency-blocked
- **AND** no duplicate operator-visible warn/error diagnostic for the same signature is appended

#### Scenario: Changed blocker signature emits a fresh diagnostic

- **GIVEN** queued change `alpha` was previously blocked by dependency `beta [missing]`
- **WHEN** repository-visible evidence changes so `beta` is now `rejected`
- **THEN** the scheduler emits a fresh diagnostic for `beta [rejected]`
- **AND** dispatch remains blocked

#### Scenario: Archived blocker re-evaluates dispatch eligibility

- **GIVEN** queued change `alpha` was previously blocked by dependency `beta [queued]`
- **WHEN** repository-visible evidence changes so `beta` is archived
- **THEN** the scheduler treats `beta` as satisfied
- **AND** `alpha` becomes eligible for dispatch if no other unresolved dependency blockers remain

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

Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace. The acceptance loop SHALL parse stdout to determine pass/fail/continue/stalled-hold outcomes, and MUST NOT use exit code as the acceptance verdict when a canonical verdict is present.

Acceptance execution MUST NOT create a workspace-root `ACCEPTANCE_REPORT.json` artifact for PASS, command failure, FAIL, CONTINUE, or stalled-hold outcomes. Acceptance outcomes MAY be recorded in in-memory runtime history, events, or non-authoritative observability logs, but the runtime MUST NOT write a new workspace-root report file as acceptance completion evidence.

Archive and resume routing MUST NOT depend on `ACCEPTANCE_REPORT.json`. The absence of that file MUST NOT prevent existing acceptance history recording, event emission, or subsequent routing behavior that is otherwise valid for the workspace file/git state.

When acceptance returns a stalled-hold compatibility verdict for infrastructure, external dependency, missing non-mockable credential, or pending-verification evidence, parallel execution SHALL record a non-terminal stalled hold and SHALL NOT invoke terminal rejection flow solely because of that verdict.

#### Scenario: acceptance pass does not create report artifact

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the acceptance command emits a canonical PASS verdict
- **WHEN** acceptance finalizes the pass
- **THEN** the runtime records the acceptance attempt in existing acceptance history
- **AND** the workspace root does not contain `ACCEPTANCE_REPORT.json`
- **AND** the pass result does not require any workspace-root report file for later routing

#### Scenario: command failure does not create misleading pass report

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the acceptance command exits unsuccessfully without a finalized canonical verdict
- **WHEN** acceptance returns a command-failure result
- **THEN** the runtime records the failed attempt through existing history paths
- **AND** the workspace root does not contain `ACCEPTANCE_REPORT.json`
- **AND** no JSON artifact is written that could describe the failed attempt as `pass`

#### Scenario: non-pass verdicts do not create report artifact

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the final acceptance verdict is FAIL, CONTINUE, or a stalled-hold compatibility verdict
- **WHEN** acceptance finalizes that outcome
- **THEN** the workspace root does not contain `ACCEPTANCE_REPORT.json`
- **AND** any required follow-up information is carried by the existing result, findings, events, or history rather than a workspace-root report file

#### Scenario: infrastructure stalled hold does not become terminal rejection

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the change is structurally valid and active
- **AND** acceptance emits a stalled-hold compatibility verdict because a required Docker image pull failed with DNS or network timeout and the image is not locally available
- **WHEN** parallel dispatch handles the acceptance result
- **THEN** `alpha` is recorded as a non-terminal stalled hold
- **AND** base branch `openspec/changes/alpha/REJECTED.md` is not created
- **AND** terminal rejection flow is not invoked solely from the infrastructure blocker
- **AND** the worktree remains available for later resume after Docker image or network availability is restored

#### Scenario: pending managed verification job is non-terminal

- **GIVEN** a required acceptance verification uses a managed job such as agent-exec
- **AND** the job is still running or lacks terminal pass/fail evidence
- **WHEN** acceptance or dispatch classifies the verification state
- **THEN** the change is not marked as accepted
- **AND** the change is not terminally rejected
- **AND** the condition is recorded as pending verification or a non-terminal stalled hold with next action to re-check the job or wait for terminal evidence

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

queuedのchangeが空の場合、analysisを実行してはならない（MUST）。ただし reducer-visible queued intent が存在する場合、scheduler は queued が空であると結論する前に reconciliation を試みなければならない（MUST）。

re-analysis は完了イベントに依存せず、キュー変化やタイマーなどのトリガで起動可能でなければならない（MUST）。

re-analysis はメインの実行ループ進行に依存せず開始できなければならない（MUST）。

スロットが空いていない場合でも re-analysis は実行でき、空きができた時点で次のディスパッチが行われなければならない（MUST）。

Scheduler reconciliation は reducer-visible queued work が analysis 対象へ取り込まれない理由を観測可能にしなければならない（SHALL）。ただし、同じ change と同じ理由が scheduler loop ごとに連続する場合、user-visible logs と WARN-level debug log entries への出力は dedupe、rate-limit、または summary 化されなければならない（SHALL）。

#### Scenario: missing queued candidate diagnostic is bounded

- **GIVEN** reducer-visible queued intent exists for change `alpha`
- **AND** `alpha` is not loadable from active OpenSpec changes
- **WHEN** scheduler reconciliation runs repeatedly without any relevant state change
- **THEN** `alpha` is not added to scheduler-local queued candidates
- **AND** the `candidate_not_found` reason remains observable at least once or through a summary
- **AND** identical `candidate_not_found` user-visible log entries are not emitted on every scheduler loop
- **AND** identical `candidate_not_found` WARN-level debug log entries are not emitted on every scheduler loop

#### Scenario: loadable queued candidate still reconciles after missing-candidate logging

- **GIVEN** reducer-visible queued intent exists for change `beta`
- **AND** `beta` is loadable from active OpenSpec changes
- **WHEN** scheduler reconciliation evaluates queued candidates
- **THEN** `beta` is added to scheduler-local queued candidates when no active, in-flight, terminal, slot, or debounce condition blocks it
- **AND** missing-candidate diagnostic suppression state does not prevent `beta` from being analyzed

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

Parallel execution SHALL continue to track failed changes for dependency-skip decisions, but the failure-side terminology MUST distinguish queue-side dependency blocking from resumable execution holds.

A change held because apply cannot proceed yet but remains resumable SHALL be recorded as `stalled`, not `blocked`.

Dependency-based queue waiting SHALL continue to use `blocked` only for unresolved dependency conditions that prevent dispatch.

#### Scenario: stalled apply blocker is recorded as failed without using blocked terminology
- **GIVEN** apply output contains a resumable blocker such as permission auto-reject
- **WHEN** the runtime records the failed change for downstream dependency-skip logic
- **THEN** the change is recorded as `stalled`
- **AND** dependent changes are still eligible for failure-based skip logic
- **AND** the user-facing wording does not describe the change as dependency `blocked`

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

依存制約が解決した change は、依存解決後の実行開始時点で worktree を新規作成し、既存の worktree がある場合も作り直さなければならない（MUST）。この dependency-resolved recreation rule は通常 resume の例外として扱われ、依存に無関係な resumed worktree reuse を一般に禁止してはならない（MUST NOT）。

runtime は dependency blocked だった change が resolved になったことを記録し、次回 dispatch では generic resume ではなく forced fresh workspace creation を選択しなければならない（MUST）。既存 worktree/branch が存在する場合、それらは fresh dispatch 前に cleanup または equivalent invalidation され、stale worktree が再利用 source として残ってはならない（MUST NOT）。

#### Scenario: dependency-resolved change recreates worktree even when one already exists
- **GIVEN** change `beta` was previously blocked waiting for dependency `alpha`
- **AND** `beta` already has an older worktree created before `alpha` was merged
- **AND** dependency `alpha` is now resolved on the base branch
- **WHEN** the scheduler dispatches `beta` for its next execution start
- **THEN** the runtime does not reuse the older worktree
- **AND** the runtime creates a fresh worktree for `beta`
- **AND** the older worktree is cleaned up or otherwise invalidated before it can be reused

#### Scenario: normal resume still reuses worktree when dependency recreation rule does not apply
- **GIVEN** change `gamma` has an existing consistent worktree
- **AND** `gamma` was not previously blocked by unresolved dependencies
- **WHEN** the scheduler resumes `gamma`
- **THEN** the runtime may reuse the existing worktree
- **AND** dependency-resolved forced recreation is not triggered solely because resume occurred

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

### Requirement: Dependency-resolved change recreates workspace once

dependency blocked だった change が `DependencyResolved` になった直後の最初の dispatch では、システムは既存 workspace を再利用せず fresh workspace を作成しなければならない（MUST）。

この dependency-resolved workspace recreation は通常 resume の例外としてのみ適用され、依存解決と無関係な通常 resume に対しては既存 workspace 再利用を禁止してはならない（MUST NOT）。

#### Scenario: dependency resolved change recreates workspace instead of resume
- **GIVEN** change `B` は dependency blocked 状態から `DependencyResolved` へ遷移した
- **AND** `B` に対応する既存 workspace が存在する
- **WHEN** scheduler が `B` を次に dispatch する
- **THEN** システムは `find_existing_workspace()` / `reuse_workspace()` で既存 workspace を再利用しない
- **AND** fresh workspace を新規作成して apply pipeline を開始する

#### Scenario: regular resume still reuses workspace
- **GIVEN** change `C` は dependency blocked を経由せず通常の resume 対象である
- **AND** `C` に対応する既存 workspace が存在する
- **WHEN** scheduler が `C` を dispatch する
- **THEN** システムは既存 workspace を再利用して `WorkspaceResumed` を発行できる
- **AND** dependency-resolved 例外を理由に強制再作成してはならない

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

re-analysis の起動トリガは、キュー通知・デバウンスタイマー・in-flight 完了・reducer-visible queued intent reconciliation のいずれでもよい（MUST）。

利用可能スロットが 0 の場合、システムは re-analysis を実行せず、空きができた時点で re-analysis を再評価しなければならない（MUST）。

スケジューラは reducer-visible queued work が存在するのに re-analysis を開始しない場合、その理由を観測可能なログまたはイベントとして出力しなければならない（SHALL）。

#### Scenario: キュー変化でre-analysisが起動する
- **GIVEN** apply 実行中の change が存在する
- **AND** queued に新しい change が追加される
- **WHEN** 並列実行が re-analysis を評価する
- **THEN** apply 完了を待たずに re-analysis が開始される

#### Scenario: reducer queued intentでre-analysisが起動する
- **GIVEN** reducer state に queued intent を持つ change が存在する
- **AND** scheduler-local queued list にはその change が存在しない
- **AND** 利用可能なスロットが1以上である
- **WHEN** 並列実行が re-analysis を評価する
- **THEN** scheduler は reducer-visible queued intent を analysis candidate に取り込む
- **AND** dynamic queue notification だけに依存せず re-analysis を開始する

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
- **AND** no available slots の理由がログまたはイベントで観測できる

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

scheduler-local queued 集合は reducer-visible queued intent と reconcile されなければならない（MUST）。reconcile は dynamic queue notification の欠落、dynamic queue pop 後の一時的な candidate load failure、または stale local queue state によって reducer-visible queued change が永続的に analysis 対象外になることを防がなければならない（MUST）。

queued の change が空の場合、analysis を実行してはならない（MUST）。ただし reducer-visible queued intent が存在する場合、scheduler は queued が空であると結論する前に reconcile を試みなければならない（MUST）。

実行中の change が存在せず、queued の change も空の場合、オーケストレーションは完了状態にならなければならない（MUST）。ただし reducer-visible queued intent が存在する場合、その intent が terminal / active / missing などの理由で analysis 対象外であることが確認されるまで完了状態として扱ってはならない（MUST NOT）。

queued に含まれない change（例: merged 済み change、実行済み change、削除済み change）は analysis 対象から除外されなければならない（MUST）。

Archived-dirty repair candidate は workspace-derived repair trigger として扱われなければならない（MUST）。scheduler は同じ unchanged archived-dirty repair candidate の再発見を通常の user/reducer queue addition と同じ debounce 更新として扱ってはならない（MUST NOT）。

#### Scenario: queuedのみがanalysis対象になる

- **GIVEN** queued に change が存在する
- **AND** queued 以外に実行中の change が存在する
- **WHEN** 並列実行が analysis を開始する
- **THEN** analysis 対象は queued の change のみになる

#### Scenario: reducer queued intent が scheduler-local queued に反映される

- **GIVEN** change `beta` has reducer-visible queued intent
- **AND** `beta` is not terminal
- **AND** `beta` is not active or in-flight
- **AND** `beta` can be loaded from active OpenSpec changes
- **AND** scheduler-local queued list does not contain `beta`
- **WHEN** the scheduler reconciles queued candidates before analysis
- **THEN** `beta` is added to scheduler-local queued candidates
- **AND** the next analysis includes `beta`

#### Scenario: dynamic queue notification miss is recoverable

- **GIVEN** change `beta` has reducer-visible queued intent
- **AND** the dynamic queue notification for `beta` was missed or already popped
- **AND** scheduler-local queued list does not contain `beta`
- **WHEN** the scheduler loop next reconciles queued candidates
- **THEN** `beta` is still eligible for analysis through reducer-visible queued intent
- **AND** `beta` does not remain indefinitely queued without analysis solely because the notification was missed

#### Scenario: candidate load failure is observable and retried

- **GIVEN** dynamic queue ingestion sees queued change id `beta`
- **AND** active OpenSpec change loading does not currently return `beta`
- **WHEN** scheduler ingestion skips `beta`
- **THEN** the skip reason is logged or emitted as candidate not found
- **AND** if reducer-visible queued intent for `beta` remains and `beta` later becomes loadable, reconciliation can add `beta` to analysis candidates

#### Scenario: queuedが空ならanalysisを実行しない

- **GIVEN** queued の change が存在しない
- **AND** reducer-visible queued intent も存在しない
- **WHEN** 並列実行が analysis を開始しようとする
- **THEN** analysis を実行しない

#### Scenario: 実行中とqueuedが空なら終了する

- **GIVEN** 実行中の change が存在しない
- **AND** queued の change も空である
- **AND** reducer-visible queued intent も存在しない
- **WHEN** 並列実行ループが次の analysis を開始しようとする
- **THEN** analysis を実行しない
- **AND** オーケストレーションは完了状態になる

#### Scenario: queued外のchangeはanalysis対象から除外される

- **GIVEN** queued に含まれない change が存在する
- **AND** queued には別の change が存在する
- **WHEN** 並列実行が analysis を開始する
- **THEN** queued 外の change は analysis 対象から除外される

#### Scenario: archived dirty repair candidate does not extend debounce indefinitely

- **GIVEN** reducer-visible queued intent is empty
- **AND** an existing worktree for change `alpha` has no active `openspec/changes/alpha` directory
- **AND** the same worktree has an archive entry for `alpha`
- **AND** scheduler reconciliation discovers `alpha` as an archived-dirty repair candidate
- **WHEN** scheduler reconciliation observes the same unchanged repair candidate repeatedly
- **THEN** the scheduler MUST NOT refresh normal queue debounce on every loop for `alpha`
- **AND** repair-driven analysis for `alpha` MUST either bypass debounce or run after one bounded debounce interval
- **AND** analysis MUST NOT be postponed indefinitely by rediscovering `alpha` itself

#### Scenario: repeated unchanged repair reconciliation is bounded

- **GIVEN** scheduler reconciliation observes the same archived-dirty repair candidate set repeatedly
- **AND** no dispatch, completion, merge, archive, resolve, queue addition, or worktree state change occurs
- **WHEN** the scheduler loop evaluates queued candidates multiple times
- **THEN** repeated user-visible repair reconciliation diagnostics MUST be deduped, rate-limited, or summarized
- **AND** unchanged repair rediscovery MUST NOT be treated as new scheduler progress each time
- **AND** the scheduler MUST remain capable of progressing analysis when execution capacity is available

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

When permission auto-reject is detected during apply, the system MUST stop apply retry for that change and record the change as `stalled`.

The system MUST NOT label this condition as dependency `blocked`.

#### Scenario: permission auto-reject becomes stalled
- **GIVEN** apply output contains `permission requested` and `auto-rejecting`
- **WHEN** the apply loop evaluates the output
- **THEN** the change is recorded as `stalled`
- **AND** apply retry does not continue
- **AND** stall detection via empty WIP commits is skipped for that change
- **AND** the recorded reason includes rejected paths and permission guidance

### Requirement: Resumed Archived Workspaces Preserve Merge Handoff

When parallel execution resumes a workspace already detected as `WorkspaceState::Archived`, the executor SHALL treat that workspace as archive-complete for downstream lifecycle handling.

The resumed workspace MUST NOT silently complete in a way that bypasses merge handling or causes the change to regress to `NotQueued` before merge resolution is attempted.

Queue reconciliation MUST NOT rediscover an archived or archived-dirty worktree as scheduler-local queued work when workspace-local Git/base-branch evidence shows that the same change is already merged into the base branch.

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

#### Scenario: Already merged archived worktree is terminal residue

- **GIVEN** a leftover parallel worktree contains an archived change entry for `alpha`
- **AND** workspace-local Git/base-branch comparison shows `alpha` is already merged into the base branch
- **WHEN** scheduler queue reconciliation scans existing worktrees
- **THEN** the worktree is not added to scheduler-local queued work as an archived-dirty repair candidate
- **AND** apply, acceptance, and archive are not run for `alpha`
- **AND** no user-visible archived-dirty repair diagnostic is emitted for `alpha`

#### Scenario: Non-merged archived dirty worktree remains repairable

- **GIVEN** a leftover parallel worktree contains an archived change entry for `beta`
- **AND** workspace-local Git/base-branch comparison does not show `beta` as merged
- **AND** the worktree represents an interrupted archive-finalization path
- **WHEN** scheduler queue reconciliation scans existing worktrees
- **THEN** `beta` may be added as an archived-dirty repair candidate
- **AND** the normal archive-complete merge handoff or repair path remains available

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

When parallel merge verification runs after archive completion, a change that is already integrated into the base branch via fast-forward SHALL be treated as merged rather than as a merge verification failure.

#### Scenario: archive-complete change fast-forwarded during parallel merge does not fail verification

**Given** a change completed archive successfully in parallel mode
**And** the subsequent merge path integrates the change into the base branch via fast-forward
**When** post-merge verification checks for merge completion
**Then** the change is treated as merged
**And** the system does not emit a merge verification error based only on missing merge commit subject

### Requirement: Parallel execution acceptance loop

Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace. The acceptance loop SHALL parse stdout to determine pass/fail/continue/stalled-hold outcomes, and MUST NOT use exit code as the acceptance verdict when a canonical verdict is present.

Acceptance execution MUST NOT create a workspace-root `ACCEPTANCE_REPORT.json` artifact for PASS, command failure, FAIL, CONTINUE, or stalled-hold outcomes. Acceptance outcomes MAY be recorded in in-memory runtime history, events, or non-authoritative observability logs, but the runtime MUST NOT write a new workspace-root report file as acceptance completion evidence.

Archive and resume routing MUST NOT depend on `ACCEPTANCE_REPORT.json`. The absence of that file MUST NOT prevent existing acceptance history recording, event emission, or subsequent routing behavior that is otherwise valid for the workspace file/git state.

When acceptance returns a stalled-hold compatibility verdict for infrastructure, external dependency, missing non-mockable credential, or pending-verification evidence, parallel execution SHALL record a non-terminal stalled hold and SHALL NOT invoke terminal rejection flow solely because of that verdict.

#### Scenario: acceptance pass does not create report artifact

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the acceptance command emits a canonical PASS verdict
- **WHEN** acceptance finalizes the pass
- **THEN** the runtime records the acceptance attempt in existing acceptance history
- **AND** the workspace root does not contain `ACCEPTANCE_REPORT.json`
- **AND** the pass result does not require any workspace-root report file for later routing

#### Scenario: command failure does not create misleading pass report

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the acceptance command exits unsuccessfully without a finalized canonical verdict
- **WHEN** acceptance returns a command-failure result
- **THEN** the runtime records the failed attempt through existing history paths
- **AND** the workspace root does not contain `ACCEPTANCE_REPORT.json`
- **AND** no JSON artifact is written that could describe the failed attempt as `pass`

#### Scenario: non-pass verdicts do not create report artifact

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the final acceptance verdict is FAIL, CONTINUE, or a stalled-hold compatibility verdict
- **WHEN** acceptance finalizes that outcome
- **THEN** the workspace root does not contain `ACCEPTANCE_REPORT.json`
- **AND** any required follow-up information is carried by the existing result, findings, events, or history rather than a workspace-root report file

#### Scenario: infrastructure stalled hold does not become terminal rejection

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the change is structurally valid and active
- **AND** acceptance emits a stalled-hold compatibility verdict because a required Docker image pull failed with DNS or network timeout and the image is not locally available
- **WHEN** parallel dispatch handles the acceptance result
- **THEN** `alpha` is recorded as a non-terminal stalled hold
- **AND** base branch `openspec/changes/alpha/REJECTED.md` is not created
- **AND** terminal rejection flow is not invoked solely from the infrastructure blocker
- **AND** the worktree remains available for later resume after Docker image or network availability is restored

#### Scenario: pending managed verification job is non-terminal

- **GIVEN** a required acceptance verification uses a managed job such as agent-exec
- **AND** the job is still running or lacks terminal pass/fail evidence
- **WHEN** acceptance or dispatch classifies the verification state
- **THEN** the change is not marked as accepted
- **AND** the change is not terminally rejected
- **AND** the condition is recorded as pending verification or a non-terminal stalled hold with next action to re-check the job or wait for terminal evidence

### Requirement: Shared Parallel Orchestration Service

Parallel merge retry dispatch SHALL handle deferred merge attempts as pending/deferred outcomes rather than successful completed background merge tasks. A background merge task MUST report successful completion only when the merge attempt actually integrated the change into base or determined from repository-visible evidence that the change was already integrated.

Parallel ordinary apply dispatch MUST treat reducer terminal-error state as a stop gate. After a change emits an apply, acceptance, archive, dispatch, or workspace execution error, scheduler reanalysis, queue reconciliation, and workspace resume scans MUST NOT dispatch that same change to apply again unless explicit retry intent has cleared the recoverable error terminal state. Existing workspaces MAY remain available for operator inspection or explicit retry.

Dependency analysis MUST continue to treat an errored dependency as a dispatch blocker for dependents until the dependency is explicitly retried and reaches repository-visible success.

<!-- Expected canonical result after archive: `parallel-execution` will require terminal-error changes to stay stopped across reanalysis/resume until explicit retry clears the reducer error. -->

#### Scenario: parallel apply error is not automatically redispatched

**Given**: change `alpha` is running in parallel apply
**When**: the workspace task emits `ProcessingError` or `ApplyFailed` for `alpha`
**Then**: `alpha` is recorded as `error`
**And**: the next scheduler reanalysis does not select `alpha` for ordinary apply dispatch
**And**: `alpha` remains available for explicit retry rather than being removed silently

#### Scenario: workspace resume does not resurrect errored change

**Given**: change `alpha` has terminal state `Error`
**And**: an existing workspace for `alpha` remains on disk
**When**: parallel workspace resume or repair-candidate scanning runs
**Then**: `alpha` is not dispatched to ordinary apply solely because the workspace exists
**And**: `alpha` remains displayed as `error` until explicit retry or delayed repository-visible success

#### Scenario: explicit retry restores parallel dispatch eligibility

**Given**: change `alpha` has terminal state `Error`
**And**: the operator explicitly marks `alpha` for retry
**When**: the retry transition clears the recoverable error terminal state
**Then**: `alpha` may be selected by normal parallel dependency analysis and dispatch rules
**And**: unmarked error changes remain excluded from ordinary apply dispatch

#### Scenario: errored dependency blocks dependent dispatch

**Given**: queued change `beta` depends on change `alpha`
**And**: `alpha` has terminal state `Error`
**When**: parallel dependency analysis selects dispatch candidates
**Then**: `beta` is not dispatched
**And**: after `alpha` is explicitly retried and reaches repository-visible success, `beta` may be re-evaluated by normal dependency analysis

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

After rejecting review completes, the runtime SHALL emit a `RejectionReviewCompleted` execution event with one of `Confirm`, `Resume`, or `Block` outcome. The reducer SHALL use this event to drive the `Rejecting → Rejected`, `Rejecting → Applying`, or `Rejecting → Stalled` transition.

The runtime SHALL NOT leave a change in the `Rejecting` activity stage after rejection review has produced a verdict. If rejection review encounters an error, the runtime SHALL emit a `RejectionReviewFailed` event to transition the change to `Error` terminal state.

#### Scenario: blocked rejection review emits completion event and returns to stalled state

- **GIVEN** a change is in the `Rejecting` activity stage
- **AND** rejection review returns `REJECTION_REVIEW: BLOCK`
- **WHEN** the blocking handoff completes
- **THEN** a `RejectionReviewCompleted` event with `Block` outcome is emitted
- **AND** the reducer transitions the change to non-terminal stalled state
- **AND** base branch `openspec/changes/<change_id>/REJECTED.md` is not created
- **AND** the worktree remains available for later resume

#### Scenario: confirmed rejection remains terminal

- **GIVEN** a change is in the `Rejecting` activity stage
- **AND** rejection review returns `REJECTION_REVIEW: CONFIRM` with terminal evidence that the change premise is invalid, obsolete, contradictory, or constitution-violating
- **WHEN** rejection flow completes
- **THEN** base branch `openspec/changes/<change_id>/REJECTED.md` is created
- **AND** the change is dequeued
- **AND** the reducer marks the change terminal rejected

### Requirement: ParallelRunService rejection flow on blocked execution

After rejecting review completes, the runtime SHALL emit a `RejectionReviewCompleted` execution event with one of `Confirm`, `Resume`, or `Block` outcome. The reducer SHALL use this event to drive the `Rejecting → Rejected`, `Rejecting → Applying`, or `Rejecting → Stalled` transition.

The runtime SHALL NOT leave a change in the `Rejecting` activity stage after rejection review has produced a verdict. If rejection review encounters an error, the runtime SHALL emit a `RejectionReviewFailed` event to transition the change to `Error` terminal state.

#### Scenario: blocked rejection review emits completion event and returns to stalled state

- **GIVEN** a change is in the `Rejecting` activity stage
- **AND** rejection review returns `REJECTION_REVIEW: BLOCK`
- **WHEN** the blocking handoff completes
- **THEN** a `RejectionReviewCompleted` event with `Block` outcome is emitted
- **AND** the reducer transitions the change to non-terminal stalled state
- **AND** base branch `openspec/changes/<change_id>/REJECTED.md` is not created
- **AND** the worktree remains available for later resume

#### Scenario: confirmed rejection remains terminal

- **GIVEN** a change is in the `Rejecting` activity stage
- **AND** rejection review returns `REJECTION_REVIEW: CONFIRM` with terminal evidence that the change premise is invalid, obsolete, contradictory, or constitution-violating
- **WHEN** rejection flow completes
- **THEN** base branch `openspec/changes/<change_id>/REJECTED.md` is created
- **AND** the change is dequeued
- **AND** the reducer marks the change terminal rejected

### Requirement: Parallel execution acceptance loop

Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace. The acceptance loop SHALL parse stdout to determine pass/fail/continue/stalled-hold outcomes, and MUST NOT use exit code as the acceptance verdict when a canonical verdict is present.

Acceptance execution MUST NOT create a workspace-root `ACCEPTANCE_REPORT.json` artifact for PASS, command failure, FAIL, CONTINUE, or stalled-hold outcomes. Acceptance outcomes MAY be recorded in in-memory runtime history, events, or non-authoritative observability logs, but the runtime MUST NOT write a new workspace-root report file as acceptance completion evidence.

Archive and resume routing MUST NOT depend on `ACCEPTANCE_REPORT.json`. The absence of that file MUST NOT prevent existing acceptance history recording, event emission, or subsequent routing behavior that is otherwise valid for the workspace file/git state.

When acceptance returns a stalled-hold compatibility verdict for infrastructure, external dependency, missing non-mockable credential, or pending-verification evidence, parallel execution SHALL record a non-terminal stalled hold and SHALL NOT invoke terminal rejection flow solely because of that verdict.

#### Scenario: acceptance pass does not create report artifact

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the acceptance command emits a canonical PASS verdict
- **WHEN** acceptance finalizes the pass
- **THEN** the runtime records the acceptance attempt in existing acceptance history
- **AND** the workspace root does not contain `ACCEPTANCE_REPORT.json`
- **AND** the pass result does not require any workspace-root report file for later routing

#### Scenario: command failure does not create misleading pass report

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the acceptance command exits unsuccessfully without a finalized canonical verdict
- **WHEN** acceptance returns a command-failure result
- **THEN** the runtime records the failed attempt through existing history paths
- **AND** the workspace root does not contain `ACCEPTANCE_REPORT.json`
- **AND** no JSON artifact is written that could describe the failed attempt as `pass`

#### Scenario: non-pass verdicts do not create report artifact

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the final acceptance verdict is FAIL, CONTINUE, or a stalled-hold compatibility verdict
- **WHEN** acceptance finalizes that outcome
- **THEN** the workspace root does not contain `ACCEPTANCE_REPORT.json`
- **AND** any required follow-up information is carried by the existing result, findings, events, or history rather than a workspace-root report file

#### Scenario: infrastructure stalled hold does not become terminal rejection

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the change is structurally valid and active
- **AND** acceptance emits a stalled-hold compatibility verdict because a required Docker image pull failed with DNS or network timeout and the image is not locally available
- **WHEN** parallel dispatch handles the acceptance result
- **THEN** `alpha` is recorded as a non-terminal stalled hold
- **AND** base branch `openspec/changes/alpha/REJECTED.md` is not created
- **AND** terminal rejection flow is not invoked solely from the infrastructure blocker
- **AND** the worktree remains available for later resume after Docker image or network availability is restored

#### Scenario: pending managed verification job is non-terminal

- **GIVEN** a required acceptance verification uses a managed job such as agent-exec
- **AND** the job is still running or lacks terminal pass/fail evidence
- **WHEN** acceptance or dispatch classifies the verification state
- **THEN** the change is not marked as accepted
- **AND** the change is not terminally rejected
- **AND** the condition is recorded as pending verification or a non-terminal stalled hold with next action to re-check the job or wait for terminal evidence

### Requirement: Workspace State Detection
Existing workspaces SHALL be classified from worktree state in a way that preserves canonical execution ordering for resume.

Archive-complete terminal detection MAY still use committed file state, but non-terminal resumed worktrees MUST NOT be classified into a direct-archive execution path.

When a reused worktree is not archive-complete:
- the orchestrator MUST inspect worktree-local task progress for the change
- it MUST choose `apply` when progress is below 100% or unavailable
- it MUST choose `acceptance` when progress is 100%
- it MUST NOT choose archive directly

#### Scenario: Non-terminal resumed worktree never routes directly to archive
- **GIVEN** a reused worktree is neither archive-complete nor merged
- **WHEN** resume classification is performed
- **THEN** the next execution step is either apply or acceptance
- **AND** archive is not selected as the first resumed non-terminal step

### Requirement: Queue ingestion and analysis targeting

並列実行の analysis は queued の change のみを対象にしなければならない（MUST）。

キューに追加された change は analysis 実行前に queued 集合へ反映されなければならない（MUST）。

scheduler-local queued 集合は reducer-visible queued intent と reconcile されなければならない（MUST）。reconcile は dynamic queue notification の欠落、dynamic queue pop 後の一時的な candidate load failure、または stale local queue state によって reducer-visible queued change が永続的に analysis 対象外になることを防がなければならない（MUST）。

queued の change が空の場合、analysis を実行してはならない（MUST）。ただし reducer-visible queued intent が存在する場合、scheduler は queued が空であると結論する前に reconcile を試みなければならない（MUST）。

実行中の change が存在せず、queued の change も空の場合、オーケストレーションは完了状態にならなければならない（MUST）。ただし reducer-visible queued intent が存在する場合、その intent が terminal / active / missing などの理由で analysis 対象外であることが確認されるまで完了状態として扱ってはならない（MUST NOT）。

queued に含まれない change（例: merged 済み change、実行済み change、削除済み change）は analysis 対象から除外されなければならない（MUST）。

Archived-dirty repair candidate は workspace-derived repair trigger として扱われなければならない（MUST）。scheduler は同じ unchanged archived-dirty repair candidate の再発見を通常の user/reducer queue addition と同じ debounce 更新として扱ってはならない（MUST NOT）。

#### Scenario: queuedのみがanalysis対象になる

- **GIVEN** queued に change が存在する
- **AND** queued 以外に実行中の change が存在する
- **WHEN** 並列実行が analysis を開始する
- **THEN** analysis 対象は queued の change のみになる

#### Scenario: reducer queued intent が scheduler-local queued に反映される

- **GIVEN** change `beta` has reducer-visible queued intent
- **AND** `beta` is not terminal
- **AND** `beta` is not active or in-flight
- **AND** `beta` can be loaded from active OpenSpec changes
- **AND** scheduler-local queued list does not contain `beta`
- **WHEN** the scheduler reconciles queued candidates before analysis
- **THEN** `beta` is added to scheduler-local queued candidates
- **AND** the next analysis includes `beta`

#### Scenario: dynamic queue notification miss is recoverable

- **GIVEN** change `beta` has reducer-visible queued intent
- **AND** the dynamic queue notification for `beta` was missed or already popped
- **AND** scheduler-local queued list does not contain `beta`
- **WHEN** the scheduler loop next reconciles queued candidates
- **THEN** `beta` is still eligible for analysis through reducer-visible queued intent
- **AND** `beta` does not remain indefinitely queued without analysis solely because the notification was missed

#### Scenario: candidate load failure is observable and retried

- **GIVEN** dynamic queue ingestion sees queued change id `beta`
- **AND** active OpenSpec change loading does not currently return `beta`
- **WHEN** scheduler ingestion skips `beta`
- **THEN** the skip reason is logged or emitted as candidate not found
- **AND** if reducer-visible queued intent for `beta` remains and `beta` later becomes loadable, reconciliation can add `beta` to analysis candidates

#### Scenario: queuedが空ならanalysisを実行しない

- **GIVEN** queued の change が存在しない
- **AND** reducer-visible queued intent も存在しない
- **WHEN** 並列実行が analysis を開始しようとする
- **THEN** analysis を実行しない

#### Scenario: 実行中とqueuedが空なら終了する

- **GIVEN** 実行中の change が存在しない
- **AND** queued の change も空である
- **AND** reducer-visible queued intent も存在しない
- **WHEN** 並列実行ループが次の analysis を開始しようとする
- **THEN** analysis を実行しない
- **AND** オーケストレーションは完了状態になる

#### Scenario: queued外のchangeはanalysis対象から除外される

- **GIVEN** queued に含まれない change が存在する
- **AND** queued には別の change が存在する
- **WHEN** 並列実行が analysis を開始する
- **THEN** queued 外の change は analysis 対象から除外される

#### Scenario: archived dirty repair candidate does not extend debounce indefinitely

- **GIVEN** reducer-visible queued intent is empty
- **AND** an existing worktree for change `alpha` has no active `openspec/changes/alpha` directory
- **AND** the same worktree has an archive entry for `alpha`
- **AND** scheduler reconciliation discovers `alpha` as an archived-dirty repair candidate
- **WHEN** scheduler reconciliation observes the same unchanged repair candidate repeatedly
- **THEN** the scheduler MUST NOT refresh normal queue debounce on every loop for `alpha`
- **AND** repair-driven analysis for `alpha` MUST either bypass debounce or run after one bounded debounce interval
- **AND** analysis MUST NOT be postponed indefinitely by rediscovering `alpha` itself

#### Scenario: repeated unchanged repair reconciliation is bounded

- **GIVEN** scheduler reconciliation observes the same archived-dirty repair candidate set repeatedly
- **AND** no dispatch, completion, merge, archive, resolve, queue addition, or worktree state change occurs
- **WHEN** the scheduler loop evaluates queued candidates multiple times
- **THEN** repeated user-visible repair reconciliation diagnostics MUST be deduped, rate-limited, or summarized
- **AND** unchanged repair rediscovery MUST NOT be treated as new scheduler progress each time
- **AND** the scheduler MUST remain capable of progressing analysis when execution capacity is available

### Requirement: Acceptance failure returns to apply loop

When acceptance returns FAIL, the parallel dispatch loop MUST re-enter the apply step on the next cycle, regardless of how the workspace was initially routed (fresh start or resume).

#### Scenario: Resumed workspace acceptance failure triggers apply retry

- **GIVEN** a parallel workspace resumed with state `Applied` (routed to acceptance-only on first cycle)
- **WHEN** the acceptance step returns `ACCEPTANCE: FAIL`
- **THEN** the next cycle of the apply+acceptance loop MUST execute the apply step before running acceptance again

### Requirement: ParallelRunService rejection flow on blocked execution

After rejecting review completes, the runtime SHALL emit a `RejectionReviewCompleted` execution event with one of `Confirm`, `Resume`, or `Block` outcome. The reducer SHALL use this event to drive the `Rejecting → Rejected`, `Rejecting → Applying`, or `Rejecting → Stalled` transition.

The runtime SHALL NOT leave a change in the `Rejecting` activity stage after rejection review has produced a verdict. If rejection review encounters an error, the runtime SHALL emit a `RejectionReviewFailed` event to transition the change to `Error` terminal state.

#### Scenario: blocked rejection review emits completion event and returns to stalled state

- **GIVEN** a change is in the `Rejecting` activity stage
- **AND** rejection review returns `REJECTION_REVIEW: BLOCK`
- **WHEN** the blocking handoff completes
- **THEN** a `RejectionReviewCompleted` event with `Block` outcome is emitted
- **AND** the reducer transitions the change to non-terminal stalled state
- **AND** base branch `openspec/changes/<change_id>/REJECTED.md` is not created
- **AND** the worktree remains available for later resume

#### Scenario: confirmed rejection remains terminal

- **GIVEN** a change is in the `Rejecting` activity stage
- **AND** rejection review returns `REJECTION_REVIEW: CONFIRM` with terminal evidence that the change premise is invalid, obsolete, contradictory, or constitution-violating
- **WHEN** rejection flow completes
- **THEN** base branch `openspec/changes/<change_id>/REJECTED.md` is created
- **AND** the change is dequeued
- **AND** the reducer marks the change terminal rejected

### Requirement: Parallel rejecting resume semantics

Parallel execution SHALL route rejection-review handoff through the same single base-mutating lane used by merge/resolve operations. A change that needs rejection review may enter active `Rejecting` only when no other non-terminal change is actively `Resolving` or `Rejecting`.

If the base-mutating lane is occupied, the rejection-review handoff SHALL become reducer-owned `RejectWait` and display `reject pending`. This wait is auto-resumable and MUST NOT require manual user action.

#### Scenario: rejecting handoff waits behind resolving

**Given**: Change A is actively `Resolving`
**And**: Change B apply execution records `openspec/changes/<change_id>/REJECTED.md`
**When**: parallel dispatch handles B's rejecting handoff
**Then**: B does not start rejection review immediately
**And**: B enters `RejectWait`
**And**: B displays `reject pending`
**And**: B is retried automatically after A clears the base-mutating lane

#### Scenario: rejecting handoff waits behind rejecting

**Given**: Change A is actively `Rejecting`
**And**: Change B apply execution records `openspec/changes/<change_id>/REJECTED.md`
**When**: parallel dispatch handles B's rejecting handoff
**Then**: B does not start rejection review immediately
**And**: B enters `RejectWait`
**And**: B displays `reject pending`
**And**: B is retried automatically after A's rejection review completes or fails

#### Scenario: rejecting handoff starts when lane is clear

**Given**: no non-terminal change is actively `Resolving` or `Rejecting`
**And**: Change B apply execution records `openspec/changes/<change_id>/REJECTED.md`
**When**: parallel dispatch handles B's rejecting handoff
**Then**: B enters active `Rejecting`
**And**: B displays `rejecting`
**And**: no other change is active in the base-mutating lane

### Requirement: Parallel execution acceptance loop

Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace. The acceptance loop SHALL parse stdout to determine pass/fail/continue/stalled-hold outcomes, and MUST NOT use exit code as the acceptance verdict when a canonical verdict is present.

Acceptance execution MUST NOT create a workspace-root `ACCEPTANCE_REPORT.json` artifact for PASS, command failure, FAIL, CONTINUE, or stalled-hold outcomes. Acceptance outcomes MAY be recorded in in-memory runtime history, events, or non-authoritative observability logs, but the runtime MUST NOT write a new workspace-root report file as acceptance completion evidence.

Archive and resume routing MUST NOT depend on `ACCEPTANCE_REPORT.json`. The absence of that file MUST NOT prevent existing acceptance history recording, event emission, or subsequent routing behavior that is otherwise valid for the workspace file/git state.

When acceptance returns a stalled-hold compatibility verdict for infrastructure, external dependency, missing non-mockable credential, or pending-verification evidence, parallel execution SHALL record a non-terminal stalled hold and SHALL NOT invoke terminal rejection flow solely because of that verdict.

#### Scenario: acceptance pass does not create report artifact

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the acceptance command emits a canonical PASS verdict
- **WHEN** acceptance finalizes the pass
- **THEN** the runtime records the acceptance attempt in existing acceptance history
- **AND** the workspace root does not contain `ACCEPTANCE_REPORT.json`
- **AND** the pass result does not require any workspace-root report file for later routing

#### Scenario: command failure does not create misleading pass report

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the acceptance command exits unsuccessfully without a finalized canonical verdict
- **WHEN** acceptance returns a command-failure result
- **THEN** the runtime records the failed attempt through existing history paths
- **AND** the workspace root does not contain `ACCEPTANCE_REPORT.json`
- **AND** no JSON artifact is written that could describe the failed attempt as `pass`

#### Scenario: non-pass verdicts do not create report artifact

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the final acceptance verdict is FAIL, CONTINUE, or a stalled-hold compatibility verdict
- **WHEN** acceptance finalizes that outcome
- **THEN** the workspace root does not contain `ACCEPTANCE_REPORT.json`
- **AND** any required follow-up information is carried by the existing result, findings, events, or history rather than a workspace-root report file

#### Scenario: infrastructure stalled hold does not become terminal rejection

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the change is structurally valid and active
- **AND** acceptance emits a stalled-hold compatibility verdict because a required Docker image pull failed with DNS or network timeout and the image is not locally available
- **WHEN** parallel dispatch handles the acceptance result
- **THEN** `alpha` is recorded as a non-terminal stalled hold
- **AND** base branch `openspec/changes/alpha/REJECTED.md` is not created
- **AND** terminal rejection flow is not invoked solely from the infrastructure blocker
- **AND** the worktree remains available for later resume after Docker image or network availability is restored

#### Scenario: pending managed verification job is non-terminal

- **GIVEN** a required acceptance verification uses a managed job such as agent-exec
- **AND** the job is still running or lacks terminal pass/fail evidence
- **WHEN** acceptance or dispatch classifies the verification state
- **THEN** the change is not marked as accepted
- **AND** the change is not terminally rejected
- **AND** the condition is recorded as pending verification or a non-terminal stalled hold with next action to re-check the job or wait for terminal evidence

### Requirement: Parallel execution acceptance loop

Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace. The acceptance loop SHALL parse stdout to determine pass/fail/continue/stalled-hold outcomes, and MUST NOT use exit code as the acceptance verdict when a canonical verdict is present.

Acceptance execution MUST NOT create a workspace-root `ACCEPTANCE_REPORT.json` artifact for PASS, command failure, FAIL, CONTINUE, or stalled-hold outcomes. Acceptance outcomes MAY be recorded in in-memory runtime history, events, or non-authoritative observability logs, but the runtime MUST NOT write a new workspace-root report file as acceptance completion evidence.

Archive and resume routing MUST NOT depend on `ACCEPTANCE_REPORT.json`. The absence of that file MUST NOT prevent existing acceptance history recording, event emission, or subsequent routing behavior that is otherwise valid for the workspace file/git state.

When acceptance returns a stalled-hold compatibility verdict for infrastructure, external dependency, missing non-mockable credential, or pending-verification evidence, parallel execution SHALL record a non-terminal stalled hold and SHALL NOT invoke terminal rejection flow solely because of that verdict.

#### Scenario: acceptance pass does not create report artifact

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the acceptance command emits a canonical PASS verdict
- **WHEN** acceptance finalizes the pass
- **THEN** the runtime records the acceptance attempt in existing acceptance history
- **AND** the workspace root does not contain `ACCEPTANCE_REPORT.json`
- **AND** the pass result does not require any workspace-root report file for later routing

#### Scenario: command failure does not create misleading pass report

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the acceptance command exits unsuccessfully without a finalized canonical verdict
- **WHEN** acceptance returns a command-failure result
- **THEN** the runtime records the failed attempt through existing history paths
- **AND** the workspace root does not contain `ACCEPTANCE_REPORT.json`
- **AND** no JSON artifact is written that could describe the failed attempt as `pass`

#### Scenario: non-pass verdicts do not create report artifact

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the final acceptance verdict is FAIL, CONTINUE, or a stalled-hold compatibility verdict
- **WHEN** acceptance finalizes that outcome
- **THEN** the workspace root does not contain `ACCEPTANCE_REPORT.json`
- **AND** any required follow-up information is carried by the existing result, findings, events, or history rather than a workspace-root report file

#### Scenario: infrastructure stalled hold does not become terminal rejection

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the change is structurally valid and active
- **AND** acceptance emits a stalled-hold compatibility verdict because a required Docker image pull failed with DNS or network timeout and the image is not locally available
- **WHEN** parallel dispatch handles the acceptance result
- **THEN** `alpha` is recorded as a non-terminal stalled hold
- **AND** base branch `openspec/changes/alpha/REJECTED.md` is not created
- **AND** terminal rejection flow is not invoked solely from the infrastructure blocker
- **AND** the worktree remains available for later resume after Docker image or network availability is restored

#### Scenario: pending managed verification job is non-terminal

- **GIVEN** a required acceptance verification uses a managed job such as agent-exec
- **AND** the job is still running or lacks terminal pass/fail evidence
- **WHEN** acceptance or dispatch classifies the verification state
- **THEN** the change is not marked as accepted
- **AND** the change is not terminally rejected
- **AND** the condition is recorded as pending verification or a non-terminal stalled hold with next action to re-check the job or wait for terminal evidence

### Requirement: ParallelRunService rejection flow on blocked execution

After rejecting review completes, the runtime SHALL emit a `RejectionReviewCompleted` execution event with one of `Confirm`, `Resume`, or `Block` outcome. The reducer SHALL use this event to drive the `Rejecting → Rejected`, `Rejecting → Applying`, or `Rejecting → Stalled` transition.

The runtime SHALL NOT leave a change in the `Rejecting` activity stage after rejection review has produced a verdict. If rejection review encounters an error, the runtime SHALL emit a `RejectionReviewFailed` event to transition the change to `Error` terminal state.

#### Scenario: blocked rejection review emits completion event and returns to stalled state

- **GIVEN** a change is in the `Rejecting` activity stage
- **AND** rejection review returns `REJECTION_REVIEW: BLOCK`
- **WHEN** the blocking handoff completes
- **THEN** a `RejectionReviewCompleted` event with `Block` outcome is emitted
- **AND** the reducer transitions the change to non-terminal stalled state
- **AND** base branch `openspec/changes/<change_id>/REJECTED.md` is not created
- **AND** the worktree remains available for later resume

#### Scenario: confirmed rejection remains terminal

- **GIVEN** a change is in the `Rejecting` activity stage
- **AND** rejection review returns `REJECTION_REVIEW: CONFIRM` with terminal evidence that the change premise is invalid, obsolete, contradictory, or constitution-violating
- **WHEN** rejection flow completes
- **THEN** base branch `openspec/changes/<change_id>/REJECTED.md` is created
- **AND** the change is dequeued
- **AND** the reducer marks the change terminal rejected

### Requirement: State-Driven Reanalysis Scheduling

The parallel scheduler SHALL treat `MergeWait` retry requests and queued change dispatch as scheduler-owned state transitions derived from observable reducer / scheduler state, rather than as direct TUI execution side effects.

A user pressing `M` for a `MergeWait` change MUST register retry intent that becomes visible to the scheduler, but MUST NOT by itself execute `resolve_deferred_merge(...)` or any equivalent merge / resolve operation outside the scheduler loop.

When execution slots remain available, queued changes and retry-eligible `MergeWait` changes MUST be evaluated within the same scheduler loop. A retry intent for one change MUST NOT suppress dependency re-analysis or dispatch of another queued change when the normal re-analysis conditions are satisfied.

Completion of a scheduler-owned merge / resolve retry MUST feed back into the same completion semantics used for ordinary scheduler progress, so that re-analysis and dispatch resume from scheduler state rather than from a TUI-only notify side effect.

When a user registers manual retry intent while other apply/archive/resolve work is in flight, the scheduler MUST preserve the reducer-owned `ResolveWait`, continue unrelated apply/archive progress, and retry the pending merge after the in-flight work releases scheduler/base-lane capacity. The pending change MUST NOT remain indefinitely in `ResolveWait` solely because unrelated apply/archive work was active at the time of the `M` keypress.

When the scheduler is running, no resolve/base-mutating operation is active, and one or more reducer-owned `ResolveWait` changes are retry-clean, the scheduler SHALL promote exactly one eligible pending retry to `resolving` during a scheduling evaluation. Other pending retries SHALL remain pending until the base-mutating lane clears again.

Canonical rule: `M` is **intent-only** (`ResolveWait` request in shared reducer state), scheduler loop is the **sole execution owner** for merge/resolve retry start, and reducer completion events (`ResolveCompleted`/`ResolveFailed`/`MergeDeferred`/`MergeCompleted`) are the **sole authority** for clearing or transitioning wait state.

<!-- Expected canonical result after archive: `parallel-execution` will require a running scheduler to promote one clean ResolveWait retry when the base-mutating lane is free. -->

#### Scenario: M key registers retry intent instead of direct execution

- **GIVEN** change `alpha` is in `MergeWait`
- **WHEN** the user presses `M`
- **THEN** the system records scheduler-visible retry intent for `alpha`
- **AND** the TUI command path does not directly execute `resolve_deferred_merge(...)`

#### Scenario: queued change still dispatches while another change is resolving

- **GIVEN** change `alpha` is already in `Resolving` and consumes one execution slot
- **AND** `max_parallelism` is greater than one so at least one slot remains available
- **AND** change `beta` is newly queued
- **AND** change `gamma` has scheduler-visible retry intent from `MergeWait`
- **WHEN** the scheduler evaluates re-analysis and dispatch from observable state
- **THEN** the scheduler may dispatch `beta` using the remaining available slot
- **AND** retry intent for `gamma` does not by itself suppress `beta` analysis or dispatch

#### Scenario: running scheduler promotes one clean ResolveWait retry

- **GIVEN** the scheduler is running
- **AND** no resolve/base-mutating operation is active
- **AND** changes `alpha` and `beta` are in reducer-owned `ResolveWait`
- **AND** retry preconditions for both are clean
- **WHEN** the scheduler evaluates pending base-mutating lane waiters
- **THEN** exactly one of `alpha` or `beta` SHALL start resolving
- **AND** the other SHALL remain `resolve pending`

#### Scenario: manual resolve intent progresses after unrelated apply completes

- **GIVEN** change `alpha` is in `MergeWait`
- **AND** change `beta` is applying or archiving in the same scheduler run
- **WHEN** the user presses `M` for `alpha`
- **THEN** the reducer records `alpha` in `ResolveWait`
- **AND** the scheduler continues `beta` apply/archive progress
- **WHEN** `beta` completes and the base-mutating lane is free
- **THEN** the scheduler retries the preserved merge for `alpha` without requiring another `M` keypress
- **AND** `alpha` does not remain indefinitely in `resolve pending` solely because `beta` was active when retry intent was registered

### Requirement: Managed worktree apply MUST run post-apply cleanup review before acceptance handoff

parallel mode で Conflux-managed isolated worktree 上の apply がタスク完了に到達したあと、worktree が dirty のままなら、システムは acceptance 開始前に post-apply cleanup review を実行しなければならない（MUST）。cleanup review が成功するまで acceptance に進めてはならない（MUST NOT）。

apply runtime が tasks.md 上の完了条件、または `REJECTED.md` による apply-blocked handoff を既に観測した run では、agent process やその子プロセスが stdout/stderr を保持したまま自然終了しなくても、システムは有限な grace period 後に当該 process group を terminate して handoff 判定へ進まなければならない（MUST）。この早期 terminate は完了条件を観測済みの場合にのみ成功相当として扱われなければならない（MUST）。

#### Scenario: Dirty managed worktree triggers cleanup review after apply completion

- **GIVEN** parallel mode の apply が managed git worktree 上で実行されている
- **AND** apply loop が tasks.md 上の完了条件を満たして終了した
- **AND** worktree に未コミット変更または未追跡ファイルが残っている
- **WHEN** orchestrator が apply 完了 handoff を判定する
- **THEN** orchestrator は acceptance を開始せず cleanup review operation を起動する
- **AND** cleanup review 成功後にのみ apply 完了を確定して acceptance に進む

#### Scenario: Clean managed worktree skips cleanup review

- **GIVEN** parallel mode の apply が managed git worktree 上で実行されている
- **AND** apply loop が tasks.md 上の完了条件を満たして終了した
- **AND** worktree が clean である
- **WHEN** orchestrator が apply 完了 handoff を判定する
- **THEN** cleanup review は不要である
- **AND** orchestrator は従来どおり apply 完了を確定して acceptance に進む

#### Scenario: Completion grace period terminates stale apply agent after tasks complete

- **GIVEN** parallel mode の apply command が tasks.md 上の完了条件を満たす変更を書き込む
- **AND** その後 agent process または子プロセスが stdout/stderr pipe を保持したまま居残り、自然終了しない
- **WHEN** orchestrator が apply 実行中に task completion を観測する
- **THEN** orchestrator は apply completion grace period を開始する
- **AND** grace period が満了しても process が終了しない場合は process group を terminate する
- **AND** 収集済みの workspace 状態に基づいて apply 完了 handoff を続行し acceptance に進む

#### Scenario: Completion grace period terminates stale apply agent after blocked handoff

- **GIVEN** parallel mode の apply command が worktree に `openspec/changes/{change_id}/REJECTED.md` を生成する
- **AND** その後 agent process または子プロセスが自然終了しない
- **WHEN** orchestrator が apply-blocked handoff を観測する
- **THEN** orchestrator は apply completion grace period を開始する
- **AND** grace period 満了後も child が残っていれば terminate する
- **AND** apply loop は rejecting review handoff として有限時間で終了する

#### Scenario: Incomplete apply does not get success-equivalent terminate treatment

- **GIVEN** parallel mode の apply command が tasks.md を未完了のままにしている
- **AND** `REJECTED.md` も生成していない
- **WHEN** agent process が終了せず inactivity timeout や terminate 対象になる
- **THEN** orchestrator はその run を apply 完了として扱ってはならない（MUST NOT）
- **AND** acceptance handoff を開始してはならない（MUST NOT）
- **AND** 従来どおり failure/retry/stall policy に従って扱う

### Requirement: キュー変更デバウンスとスロット駆動の再分析

依存制約が解決した change は、依存解決後の実行開始時点で worktree を新規作成し、既存の worktree がある場合も作り直さなければならない（MUST）。この dependency-resolved recreation rule は通常 resume の例外として扱われ、依存に無関係な resumed worktree reuse を一般に禁止してはならない（MUST NOT）。

runtime は dependency blocked だった change が resolved になったことを記録し、次回 dispatch では generic resume ではなく forced fresh workspace creation を選択しなければならない（MUST）。既存 worktree/branch が存在する場合、それらは fresh dispatch 前に cleanup または equivalent invalidation され、stale worktree が再利用 source として残ってはならない（MUST NOT）。

#### Scenario: dependency-resolved change recreates worktree even when one already exists
- **GIVEN** change `beta` was previously blocked waiting for dependency `alpha`
- **AND** `beta` already has an older worktree created before `alpha` was merged
- **AND** dependency `alpha` is now resolved on the base branch
- **WHEN** the scheduler dispatches `beta` for its next execution start
- **THEN** the runtime does not reuse the older worktree
- **AND** the runtime creates a fresh worktree for `beta`
- **AND** the older worktree is cleaned up or otherwise invalidated before it can be reused

#### Scenario: normal resume still reuses worktree when dependency recreation rule does not apply
- **GIVEN** change `gamma` has an existing consistent worktree
- **AND** `gamma` was not previously blocked by unresolved dependencies
- **WHEN** the scheduler resumes `gamma`
- **THEN** the runtime may reuse the existing worktree
- **AND** dependency-resolved forced recreation is not triggered solely because resume occurred

### Requirement: Parallel execution acceptance loop

Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace. The acceptance loop SHALL parse stdout to determine pass/fail/continue/stalled-hold outcomes, and MUST NOT use exit code as the acceptance verdict when a canonical verdict is present.

Acceptance execution MUST NOT create a workspace-root `ACCEPTANCE_REPORT.json` artifact for PASS, command failure, FAIL, CONTINUE, or stalled-hold outcomes. Acceptance outcomes MAY be recorded in in-memory runtime history, events, or non-authoritative observability logs, but the runtime MUST NOT write a new workspace-root report file as acceptance completion evidence.

Archive and resume routing MUST NOT depend on `ACCEPTANCE_REPORT.json`. The absence of that file MUST NOT prevent existing acceptance history recording, event emission, or subsequent routing behavior that is otherwise valid for the workspace file/git state.

When acceptance returns a stalled-hold compatibility verdict for infrastructure, external dependency, missing non-mockable credential, or pending-verification evidence, parallel execution SHALL record a non-terminal stalled hold and SHALL NOT invoke terminal rejection flow solely because of that verdict.

#### Scenario: acceptance pass does not create report artifact

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the acceptance command emits a canonical PASS verdict
- **WHEN** acceptance finalizes the pass
- **THEN** the runtime records the acceptance attempt in existing acceptance history
- **AND** the workspace root does not contain `ACCEPTANCE_REPORT.json`
- **AND** the pass result does not require any workspace-root report file for later routing

#### Scenario: command failure does not create misleading pass report

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the acceptance command exits unsuccessfully without a finalized canonical verdict
- **WHEN** acceptance returns a command-failure result
- **THEN** the runtime records the failed attempt through existing history paths
- **AND** the workspace root does not contain `ACCEPTANCE_REPORT.json`
- **AND** no JSON artifact is written that could describe the failed attempt as `pass`

#### Scenario: non-pass verdicts do not create report artifact

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the final acceptance verdict is FAIL, CONTINUE, or a stalled-hold compatibility verdict
- **WHEN** acceptance finalizes that outcome
- **THEN** the workspace root does not contain `ACCEPTANCE_REPORT.json`
- **AND** any required follow-up information is carried by the existing result, findings, events, or history rather than a workspace-root report file

#### Scenario: infrastructure stalled hold does not become terminal rejection

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the change is structurally valid and active
- **AND** acceptance emits a stalled-hold compatibility verdict because a required Docker image pull failed with DNS or network timeout and the image is not locally available
- **WHEN** parallel dispatch handles the acceptance result
- **THEN** `alpha` is recorded as a non-terminal stalled hold
- **AND** base branch `openspec/changes/alpha/REJECTED.md` is not created
- **AND** terminal rejection flow is not invoked solely from the infrastructure blocker
- **AND** the worktree remains available for later resume after Docker image or network availability is restored

#### Scenario: pending managed verification job is non-terminal

- **GIVEN** a required acceptance verification uses a managed job such as agent-exec
- **AND** the job is still running or lacks terminal pass/fail evidence
- **WHEN** acceptance or dispatch classifies the verification state
- **THEN** the change is not marked as accepted
- **AND** the change is not terminally rejected
- **AND** the condition is recorded as pending verification or a non-terminal stalled hold with next action to re-check the job or wait for terminal evidence

### Requirement: Applied resume uses workspace-local evidence only

Parallel execution MUST determine `Applied` resume routing from workspace-local evidence only.

For implementation changes, if implementation tasks are incomplete, resume routing MUST return to apply.

Otherwise, `Applied` MUST resume acceptance before archive unless workspace-local state is already `Archiving`.

Out-of-worktree durable state (for example under `~/.local/state/cflx/**`) MUST NOT be used as authoritative input for this decision.

#### Scenario: applied workspace resumes acceptance regardless of external durable state

- **GIVEN** a workspace is detected as `Applied`
- **AND** implementation tasks are complete
- **AND** external durable acceptance/archive state files exist or do not exist
- **WHEN** resume routing is evaluated
- **THEN** the next action is `Acceptance`
- **AND** the result is identical regardless of external state presence

#### Scenario: applied workspace with incomplete implementation tasks resumes apply

- **GIVEN** a workspace is detected as `Applied`
- **AND** implementation tasks are incomplete
- **WHEN** resume routing is evaluated
- **THEN** the next action is `Apply`
- **AND** acceptance/archive are not entered in that cycle

#### Scenario: archiving workspace resumes archive without external context

- **GIVEN** a workspace is detected as `Archiving`
- **WHEN** resume routing is evaluated
- **THEN** the next action is `Archive`
- **AND** no out-of-worktree durable state is required to continue

### Requirement: post-archive-merge-dispatch

If `on_merged` fails because the root repository is not safe for repo-mutating hook execution, such as root `.git/index.lock` contention, Conflux SHALL treat that as a hook failure that blocks merged transition when `continue_on_failure=false`.

A deferred merge caused by another active non-terminal change in `Resolving` or `Rejecting` SHALL advance into reducer-owned auto-resumable merge/resolve handling (`ResolveWait` or immediate resolving when promoted). Active `Rejecting` is included because rejection review can touch and dirty base state.

A deferred merge caused by active `Applying`, `Accepting`, `Archiving`, terminal `Rejected`, dirty base without an active base-mutating lane occupant, or other manual intervention requirement SHALL NOT be classified as automatic `ResolveWait` solely because that state exists. Dirty base and manual intervention deferrals SHALL remain in manual merge wait handling (`MergeWait`).

The implementation MUST NOT infer auto-resumable versus manual-wait behavior by parsing a human-readable deferred reason string.

#### Scenario: active resolving deferred archive promotes to resolve wait

**Given**: a change is archived in parallel mode
**And**: merge is deferred because another change is actively `Resolving`
**When**: the deferred merge result is processed
**Then**: the archived change enters auto-resumable deferred handling (`ResolveWait` or equivalent queued resolve intent)
**And**: this decision does not depend on parsing a free-form reason string

#### Scenario: active rejecting deferred archive promotes to resolve wait

**Given**: a change is archived in parallel mode
**And**: merge is deferred because another change is actively `Rejecting`
**When**: the deferred merge result is processed
**Then**: the archived change enters auto-resumable deferred handling (`ResolveWait` or equivalent queued resolve intent)
**And**: rejection review completion or failure triggers retry of deferred merge work

#### Scenario: dirty-base deferred archive stays merge wait

**Given**: a change is archived in parallel mode
**And**: merge is deferred because the base branch is dirty while no other change is actively `Resolving` or `Rejecting`
**When**: the deferred merge result is processed
**Then**: the change remains in manual merge wait handling (`MergeWait`)
**And**: it is not classified as auto-resumable

#### Scenario: root index lock contention blocks merged transition

**Given**: change `alpha` is repository-visible merged
**And**: `hooks.on_merged` runs a repo-mutating command such as `make bump-patch`
**And**: root `.git/index.lock` contention causes the hook to exit non-zero
**When**: the scheduler handles hook completion
**Then**: `alpha` does not transition to terminal `Merged`
**And**: `MergeCompleted` is not emitted for `alpha`
**And**: the operator-visible failure context includes the hook failure details

### Requirement: Parallel execution acceptance loop

Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace. The acceptance loop SHALL parse stdout to determine pass/fail/continue/stalled-hold outcomes, and MUST NOT use exit code as the acceptance verdict when a canonical verdict is present.

Acceptance execution MUST NOT create a workspace-root `ACCEPTANCE_REPORT.json` artifact for PASS, command failure, FAIL, CONTINUE, or stalled-hold outcomes. Acceptance outcomes MAY be recorded in in-memory runtime history, events, or non-authoritative observability logs, but the runtime MUST NOT write a new workspace-root report file as acceptance completion evidence.

Archive and resume routing MUST NOT depend on `ACCEPTANCE_REPORT.json`. The absence of that file MUST NOT prevent existing acceptance history recording, event emission, or subsequent routing behavior that is otherwise valid for the workspace file/git state.

When acceptance returns a stalled-hold compatibility verdict for infrastructure, external dependency, missing non-mockable credential, or pending-verification evidence, parallel execution SHALL record a non-terminal stalled hold and SHALL NOT invoke terminal rejection flow solely because of that verdict.

#### Scenario: acceptance pass does not create report artifact

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the acceptance command emits a canonical PASS verdict
- **WHEN** acceptance finalizes the pass
- **THEN** the runtime records the acceptance attempt in existing acceptance history
- **AND** the workspace root does not contain `ACCEPTANCE_REPORT.json`
- **AND** the pass result does not require any workspace-root report file for later routing

#### Scenario: command failure does not create misleading pass report

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the acceptance command exits unsuccessfully without a finalized canonical verdict
- **WHEN** acceptance returns a command-failure result
- **THEN** the runtime records the failed attempt through existing history paths
- **AND** the workspace root does not contain `ACCEPTANCE_REPORT.json`
- **AND** no JSON artifact is written that could describe the failed attempt as `pass`

#### Scenario: non-pass verdicts do not create report artifact

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the final acceptance verdict is FAIL, CONTINUE, or a stalled-hold compatibility verdict
- **WHEN** acceptance finalizes that outcome
- **THEN** the workspace root does not contain `ACCEPTANCE_REPORT.json`
- **AND** any required follow-up information is carried by the existing result, findings, events, or history rather than a workspace-root report file

#### Scenario: infrastructure stalled hold does not become terminal rejection

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the change is structurally valid and active
- **AND** acceptance emits a stalled-hold compatibility verdict because a required Docker image pull failed with DNS or network timeout and the image is not locally available
- **WHEN** parallel dispatch handles the acceptance result
- **THEN** `alpha` is recorded as a non-terminal stalled hold
- **AND** base branch `openspec/changes/alpha/REJECTED.md` is not created
- **AND** terminal rejection flow is not invoked solely from the infrastructure blocker
- **AND** the worktree remains available for later resume after Docker image or network availability is restored

#### Scenario: pending managed verification job is non-terminal

- **GIVEN** a required acceptance verification uses a managed job such as agent-exec
- **AND** the job is still running or lacks terminal pass/fail evidence
- **WHEN** acceptance or dispatch classifies the verification state
- **THEN** the change is not marked as accepted
- **AND** the change is not terminally rejected
- **AND** the condition is recorded as pending verification or a non-terminal stalled hold with next action to re-check the job or wait for terminal evidence

### Requirement: Archive retry observability is non-authoritative

Parallel archive execution MAY emit retry/resume reasons to logs/events/history for observability.

Those observability outputs MUST be treated as non-authoritative and MUST NOT control workflow routing decisions.

#### Scenario: archiving workspace resumes without external durable reason cache

- **GIVEN** change `alpha` is detected as `Archiving`
- **AND** no out-of-worktree archive reason cache is available
- **WHEN** the runtime resumes processing
- **THEN** archive resumes from workspace-local state
- **AND** absence of external reason cache does not change the selected next phase

### Requirement: Archived workspaces remain terminal even when durable archive state exists

Durable archive retry/resume state MUST NOT cause an already archived workspace to re-enter apply, acceptance, or archive.

#### Scenario: archived workspace ignores stale retry state and goes to merge handoff

- **GIVEN** change `beta` has a durable archive retry state from an earlier failed archive attempt
- **AND** current workspace file state is detected as `Archived`
- **WHEN** resume routing is performed
- **THEN** the runtime treats the workspace as terminal for archive purposes and hands it off to merge handling
- **AND** the stale durable archive retry state does not route the change back into apply, acceptance, or archive

### Requirement: Archive retry observability includes a structured primary reason

When archive is retried, resumed, or fails terminally, the runtime SHALL expose a structured primary reason plus supplemental context rather than only a generic retry/failure message.

#### Scenario: archive retry log/event names the retry reason

- **GIVEN** change `gamma` fails archive verification because the change directory still exists after the archive attempt
- **WHEN** the runtime schedules another archive retry
- **THEN** the retry log or event payload includes a primary archive reason indicating verification failure
- **AND** the payload includes a summary describing the concrete symptom
- **AND** downstream consumers do not have to infer the reason only from a generic `retrying archive command` string

### Requirement: Acceptance blocker input compatibility is distinct from lifecycle display taxonomy

When acceptance detects an implementation blocker, the system SHALL NOT expose that observation as `gated` in user-facing lifecycle or display taxonomy. The runtime SHALL treat the condition as a non-terminal stalled/review hold while preserving reason metadata such as `acceptance-gated` when the cause must be distinguished from dependency `blocked`.

The canonical machine-readable acceptance verdict parser MAY continue to accept `gated` input for compatibility. During migration, runtimes MAY continue to accept legacy `blocked` acceptance verdict input for backward compatibility. Newly authored lifecycle/status surfaces, operator-facing docs, and UI tests MUST NOT require `gated` as a display status.

If acceptance follow-up later routes the change into a resumable hold, that hold SHALL use `stalled` terminology rather than dependency `blocked` or display `gated`.

#### Scenario: canonical acceptance blocker displays as stalled
- **GIVEN** acceptance detects an implementation blocker for change `change-a`
- **WHEN** the runtime exposes the lifecycle/display status
- **THEN** the displayed status is `stalled`
- **AND** new prompts and tests do not require `gated` as a lifecycle/display term
- **AND** dependency wait remains the only `blocked` display meaning

#### Scenario: gated verdict input remains parser-compatible during migration
- **GIVEN** an acceptance integration emits `gated`
- **WHEN** a compatibility-aware runtime parses that verdict
- **THEN** the runtime interprets it as an acceptance blocker observation
- **AND** the user-facing lifecycle taxonomy describes the paused condition as `stalled`, not `gated`

#### Scenario: legacy blocked acceptance verdict remains backward compatible during migration
- **GIVEN** an older acceptance integration still emits `blocked`
- **WHEN** a compatibility-aware runtime parses that verdict
- **THEN** the runtime still interprets it as an acceptance blocker observation
- **AND** canonical user-facing taxonomy describes the paused condition as `stalled`

### Requirement: archived dependency references have explicit scheduler and validation semantics

The system SHALL classify dependency targets referenced from active change metadata into at least four categories: queued, in-flight, archived, and missing.

Proposal metadata dependencies SHALL be treated as authoritative hard dependencies by analyzer and scheduler paths. LLM analysis MAY add valid required dependency edges, but it MUST NOT remove or silently ignore dependencies parsed from proposal frontmatter or body fallback metadata.

Queued and in-flight dependency targets SHALL participate in dispatch gating and MUST prevent dependent changes from starting until the dependency is resolved on the base branch. Archived dependency targets SHALL be treated as already satisfied references and MUST NOT block dispatch, trigger terminal rejection, or be surfaced as generic JSON/parse failures. Missing dependency targets SHALL fail closed with dedicated diagnostics and MUST NOT allow the dependent change to dispatch.

#### Scenario: metadata dependency blocks while dependency is applying

- **GIVEN** active change `route` has proposal metadata dependency `policy`
- **AND** `policy` is currently in-flight applying
- **WHEN** analyzer and scheduler evaluate `route`
- **THEN** `route` remains dependency-blocked
- **AND** `route` is not dispatched to apply
- **AND** the dependency diagnostic identifies `policy` as in-flight or unresolved rather than omitting the edge

#### Scenario: single queued change preserves metadata dependency

- **GIVEN** `route` is the only queued change
- **AND** `route` has proposal metadata dependency `policy`
- **WHEN** analyzer uses a single-change fast path
- **THEN** the analysis result still contains `route -> policy`
- **AND** scheduler applies normal dependency gating before dispatching `route`

#### Scenario: fallback analysis preserves metadata dependency

- **GIVEN** `route` has proposal metadata dependency `policy`
- **AND** LLM analysis fails or is disabled
- **WHEN** fallback analysis creates an order result
- **THEN** the fallback result includes `route -> policy`
- **AND** the fallback is metadata-dependency-only rather than dependency-free

#### Scenario: archived dependency is satisfied and not rejected

- **GIVEN** active change `route` references dependency `contracts`
- **AND** `contracts` exists under `openspec/changes/archive/` with either exact or date-prefixed archive directory naming
- **WHEN** analyzer validation and scheduler dispatch checks evaluate `route`
- **THEN** `contracts` is classified as archived
- **AND** `route` is not rejected because of `contracts`
- **AND** `contracts` does not block dispatch once all non-archived dependencies are resolved
- **AND** diagnostics do not collapse the condition into generic invalid JSON or missing dependency output

#### Scenario: missing dependency fails closed

- **GIVEN** active change `route` references dependency `ghost`
- **AND** `ghost` exists neither in the queued set, nor the in-flight set, nor the archive tree
- **WHEN** analyzer validation or scheduler dispatch checks evaluate `route`
- **THEN** `ghost` is classified as missing
- **AND** `route` is not dispatched
- **AND** the diagnostic distinguishes missing dependency from archived dependency

#### Scenario: LLM cannot remove metadata dependency

- **GIVEN** active change `route` has proposal metadata dependency `policy`
- **AND** LLM analysis returns dependencies that omit `policy`
- **WHEN** Conflux parses and normalizes the analysis result
- **THEN** the normalized dependencies still include `route -> policy`
- **AND** dispatch gating uses the normalized dependency set

### Requirement: Rejecting recovery must update canonical tasks location for archived workspaces

Rejection review が `RESUME` または `BLOCK` を返した後、runtime は recovery task を current workspace 内で canonical な `tasks.md` に追記しなければならない（MUST）。

active change directory (`openspec/changes/<change_id>/tasks.md`) が存在しない場合、runtime は archived workspace entry (`openspec/changes/archive/<date>-<change_id>/tasks.md` または同等 path) を探索し、存在する archive tasks file を更新しなければならない（MUST）。

active path が無いことだけを理由に change を terminal `Error` にしてはならない（MUST NOT）。active path と archive path の両方が存在しない場合のみ、探索した path 一覧を含む実行エラーを返してよい（MAY）。

#### Scenario: archived workspace resumes from rejecting review without active tasks path

- **GIVEN** a change has already been archived in its worktree and `openspec/changes/<change_id>/tasks.md` no longer exists
- **AND** `openspec/changes/archive/<date>-<change_id>/tasks.md` exists
- **WHEN** rejecting review returns `REJECTION_REVIEW: RESUME`
- **THEN** the runtime appends the recovery task to the archived tasks file
- **AND** the change transitions back to applying rather than failing with file-not-found

#### Scenario: archived workspace blocks from rejecting review without active tasks path

- **GIVEN** a change has already been archived in its worktree and only the archived tasks file exists
- **WHEN** rejecting review returns `REJECTION_REVIEW: BLOCK`
- **THEN** the runtime appends the recovery task to the archived tasks file
- **AND** the change transitions to blocked rather than terminal error

#### Scenario: rejecting recovery reports explored paths when neither active nor archived tasks file exists

- **GIVEN** neither the active change tasks path nor any archived tasks path exists for a change
- **WHEN** rejecting recovery attempts to append a recovery task
- **THEN** the runtime returns an execution error
- **AND** the error message includes the explored active path and archive path candidates

### Requirement: Acceptance follow-up persistence failure must not override primary acceptance failure

When acceptance returns a non-pass verdict with findings, the runtime SHALL preserve that acceptance verdict as the primary outcome even if follow-up persistence into `tasks.md` degrades.

The runtime SHALL attempt to persist acceptance follow-up findings to the canonical tasks location for the workspace. If the active change tasks path does not exist, the runtime MAY explore an archived tasks location or another canonical fallback.

Failure to persist follow-up findings MUST NOT by itself convert an acceptance `FAIL` into a terminal execution `Error` unless the primary acceptance outcome itself is indeterminate.

If persistence degrades, the runtime SHALL record the explored path(s) and expose the persistence issue as supplemental warning/error context rather than replacing the acceptance diagnosis.

#### Scenario: active tasks path missing does not override acceptance fail
- **GIVEN** acceptance returns `FAIL` with findings for change `alpha`
- **AND** `openspec/changes/alpha/tasks.md` does not exist in the workspace
- **WHEN** the runtime attempts to persist acceptance follow-up findings
- **THEN** the primary outcome remains acceptance `FAIL`
- **AND** the runtime does not convert the change into terminal execution `Error` solely because the active tasks path is missing

#### Scenario: archived tasks path can receive acceptance follow-up
- **GIVEN** acceptance returns `FAIL` with findings for change `beta`
- **AND** the active tasks path is absent
- **AND** an archived tasks file for the same change exists in the workspace
- **WHEN** the runtime persists acceptance follow-up findings
- **THEN** the runtime appends the follow-up to the archived tasks file
- **AND** the primary outcome remains acceptance `FAIL`

#### Scenario: missing all tasks paths reports degradation as supplemental context
- **GIVEN** acceptance returns `FAIL` with findings for change `gamma`
- **AND** neither an active tasks path nor an archived tasks path exists
- **WHEN** the runtime attempts to persist the follow-up
- **THEN** the runtime reports which canonical paths were explored
- **AND** the persistence failure is surfaced as supplemental degradation context
- **AND** the primary acceptance diagnosis is not replaced by a generic tasks-file execution error

### Requirement: Incomplete apply does not get success-equivalent terminate treatment

A parallel-mode apply command that leaves `tasks.md` incomplete and does not produce a recognized handoff artifact MUST continue to follow failure/retry/stall policy.

When repeated retries produce consecutive empty WIP commits, the runtime MAY enter a bounded escalation phase before final stall classification, but it MUST NOT treat the run as completed solely because escalation or diagnosis occurred.

#### Scenario: incomplete apply still cannot bypass completion via escalation

- **GIVEN** parallel-mode apply leaves `tasks.md` incomplete
- **AND** no `REJECTED.md` or other success-equivalent handoff artifact exists
- **WHEN** the runtime enters escalation retries for consecutive empty WIP commits
- **THEN** the runtime still treats the change as incomplete
- **AND** acceptance/archive handoff does not begin
- **AND** final outcome remains subject to failure/retry/stall policy

### Requirement: Empty-WIP apply escalation before stall finalization

When empty WIP commits accumulate during apply for a change, the runtime SHALL be able to replace late retries with a stronger configured apply escalation command before final stall classification.

Escalation usage MUST be bounded by configuration and MUST remain runtime-ephemeral rather than durable workflow-control state.

#### Scenario: empty WIP retries switch from normal apply to escalation apply

- **GIVEN** `stall_detection.threshold = 5`
- **AND** `stall_detection.apply_escalation_after_empty_wip = 3`
- **AND** `stall_detection.apply_escalation_max_uses_per_stall = 2`
- **AND** `apply_escalation_command` is configured
- **WHEN** a change reaches its third consecutive empty WIP commit during apply retries
- **THEN** the next eligible retry uses `apply_escalation_command` instead of `apply_command`
- **AND** at most two escalation retries are used during that stall sequence

#### Scenario: empty WIP counter reset returns retry policy to normal apply

- **GIVEN** a change has already entered escalation retries for a stall sequence
- **AND** a later apply attempt produces a non-empty WIP commit
- **WHEN** the next retry decision is made
- **THEN** the empty-WIP streak resets
- **AND** escalation usage for that stall sequence resets
- **AND** subsequent retries use normal `apply_command` unless a new streak reaches the configured trigger again

### Requirement: Stall diagnosis runs once before final empty-WIP stall

When the final empty-WIP stall threshold is reached after escalation opportunities are exhausted, the runtime SHALL execute a dedicated stall diagnosis command once before returning the final stall outcome when `apply_stall_diagnose_command` is configured.

If `apply_stall_diagnose_command` is not configured, the runtime SHALL silently skip diagnosis and proceed with the existing final stall behavior.

Diagnosis output is supplemental evidence only and MUST NOT replace the primary empty-WIP stall reason.

#### Scenario: diagnosis runs once before final stall

- **GIVEN** a change has exhausted its allowed escalation retries
- **AND** the consecutive empty WIP count reaches the configured final stall threshold
- **AND** `apply_stall_diagnose_command` is configured
- **WHEN** the runtime finalizes the empty-WIP stall
- **THEN** it executes `apply_stall_diagnose_command` exactly once
- **AND** it records diagnosis output as diagnostic evidence/logging
- **AND** the final stall outcome still reports the empty-WIP stall as the primary reason

#### Scenario: diagnose failure does not hide the original stall cause

- **GIVEN** the runtime reaches final empty-WIP stall classification
- **AND** `apply_stall_diagnose_command` fails
- **WHEN** the runtime reports the result
- **THEN** the original empty-WIP stall remains the primary failure/outcome reason
- **AND** diagnose failure is surfaced only as supplemental warning/error evidence

#### Scenario: unset escalation or diagnose commands preserve current behavior

- **GIVEN** the runtime uses the existing stall detector configuration without new optional commands
- **WHEN** consecutive empty WIP commits reach the final threshold
- **THEN** the runtime behaves exactly as before this change
- **AND** no escalation or diagnosis command is attempted
- **AND** no extra warning is emitted solely because the optional commands are unset

#### Scenario: missing escalation command silently falls back to normal apply

- **GIVEN** `stall_detection.apply_escalation_after_empty_wip = 3`
- **AND** `stall_detection.apply_escalation_max_uses_per_stall = 2`
- **AND** `apply_escalation_command` is not configured
- **WHEN** a change reaches the escalation boundary during consecutive empty-WIP retries
- **THEN** the runtime continues using normal `apply_command`
- **AND** it does not emit a warning solely for the missing escalation command

#### Scenario: missing diagnose command silently falls back to direct final stall

- **GIVEN** a change reaches final empty-WIP stall classification
- **AND** `apply_stall_diagnose_command` is not configured
- **WHEN** the runtime finalizes the stall
- **THEN** it skips the diagnose phase
- **AND** it emits the same final stall outcome as the legacy flow
- **AND** it does not emit a warning solely for the missing diagnose command

### Requirement: Archive commit finalization retries repairable failures

After a parallel archive command has successfully moved a change from `openspec/changes/<change_id>/` into `openspec/changes/archive/`, the runtime SHALL use a bounded archive commit finalization retry loop before returning terminal archive error.

Archive commit finalization SHALL include creation or verification of a clean `Archive: <change_id>` commit. Failures from git hooks, formatter hooks, clippy hooks, direct commit failures, final archive verification failures, or hook-modified files SHALL be treated as repairable until the finalization retry budget is exhausted.

The retry loop MUST NOT depend on durable workflow-control state outside the workspace. Retry decisions MUST be derived from workspace file state, workspace git state, base/archive verification, and in-memory attempt context from the current run.

#### Scenario: hook failure during archive commit is retried

- **GIVEN** parallel archive has moved `alpha` into `openspec/changes/archive/2026-05-08-alpha/`
- **AND** the first direct `Archive: alpha` commit fails because a pre-commit hook or clippy check fails
- **WHEN** archive commit finalization evaluates the failure
- **THEN** Conflux schedules another archive commit finalization attempt before returning terminal error
- **AND** the next attempt receives the previous hook stderr as context
- **AND** the change is not marked errored solely because the first archive commit attempt failed

#### Scenario: hook-modified files are restaged and retried

- **GIVEN** archive commit finalization runs `git commit -m "Archive: alpha"`
- **AND** a pre-commit hook modifies files and exits non-zero
- **WHEN** the finalization retry loop continues
- **THEN** Conflux re-checks `git status --porcelain`
- **AND** Conflux re-stages modified files before a later archive commit attempt
- **AND** finalization can succeed if the later attempt produces a clean `Archive: alpha` commit

#### Scenario: finalization resolve can fix compile or module errors

- **GIVEN** archive commit finalization fails with stderr showing a repairable source error such as an unresolved module import
- **WHEN** Conflux invokes a subsequent archive-finalization resolve attempt
- **THEN** the resolve prompt includes the prior stderr and current git status
- **AND** if the resolve attempt fixes the source error and creates a valid archive commit, archive completes successfully

#### Scenario: archive command is not rerun when only commit finalization is incomplete

- **GIVEN** archive file movement has already succeeded for `alpha`
- **AND** only the final archive commit remains incomplete
- **WHEN** the finalization retry loop runs
- **THEN** Conflux retries archive commit finalization rather than re-running the full archive command unnecessarily
- **AND** it still revalidates that the active change directory remains absent and the archive entry remains present

#### Scenario: terminal archive error waits for finalization retry exhaustion

- **GIVEN** archive commit finalization repeatedly fails for `alpha`
- **AND** the bounded finalization retry budget is exhausted
- **WHEN** Conflux reports terminal archive failure
- **THEN** the error identifies archive commit finalization as the failed phase
- **AND** the error includes the last actionable blocker from direct commit, hook stderr, resolve output, or archive completion verification

#### Scenario: finalization retry events are visible

- **GIVEN** archive commit finalization needs another attempt after a failed commit or failed verification
- **WHEN** the retry is scheduled
- **THEN** Conflux emits a user-visible log or event that distinguishes archive commit finalization retry from archive command retry
- **AND** the event includes the attempt number, bounded retry limit, and a concise reason

### Requirement: Permission Auto-Reject Handling

When permission or local policy denial is detected during apply, the system SHALL distinguish transient/progressing denials from repeated unresolved denials.

A permission/policy denial SHALL become a non-terminal `stalled` hold only after the same unresolved denial recurs without repository-visible progress that would indicate the agent can continue self-healing within the workspace.

The system MUST NOT label this condition as dependency `blocked`.

#### Scenario: first permission auto-reject remains retryable

- **GIVEN** apply output contains `permission requested` and `auto-rejecting`
- **AND** this is the first observation of that denial signature for the current apply/acceptance cycle
- **WHEN** the apply loop evaluates the output
- **THEN** the change is not immediately recorded as `stalled`
- **AND** the runtime may retry according to existing apply retry policy
- **AND** the denial signature is recorded only as non-authoritative observability/retry context unless repository-visible state later makes the stalled hold derivable

#### Scenario: permission auto-reject with progress remains retryable

- **GIVEN** apply output contains a permission/policy denial
- **AND** task progress, tracked workspace files, or other repository-visible progress changed after the attempt
- **WHEN** the apply loop evaluates the output
- **THEN** the change is not recorded as `stalled` solely because the denial occurred
- **AND** apply retry may continue because the agent may still be self-healing within the workspace

#### Scenario: repeated unresolved permission auto-reject becomes stalled

- **GIVEN** apply output contains a permission/policy denial with the same denied target or equivalent denial signature as a prior attempt
- **AND** no repository-visible progress occurred between the repeated denial observations
- **WHEN** the apply loop evaluates the repeated unresolved denial
- **THEN** the change is recorded as `stalled`
- **AND** apply retry does not continue for that denial
- **AND** stall detection via empty WIP commits is skipped for that change once the repeated permission blocker is classified
- **AND** the recorded reason includes rejected paths or commands and permission guidance

### Requirement: Acceptance Permission Denial Handling

When permission or local policy denial is detected during acceptance, the system SHALL distinguish ordinary acceptance failures from repeated unresolved permission/policy blockers.

A permission/policy denial in acceptance command output, command error text, or FAIL findings SHALL become a non-terminal `stalled` hold only after the same unresolved denial recurs without repository-visible progress or changed acceptance evidence that would indicate the agent can continue self-healing.

Normal acceptance failures that do not match a repeated unresolved permission/policy denial MUST continue to use the existing acceptance follow-up and apply retry behavior.

#### Scenario: first acceptance permission denial remains retryable or reportable

- **GIVEN** acceptance output, command error text, or FAIL findings contain a permission/policy denial
- **AND** this is the first observation of that denial signature for the current acceptance follow-up cycle
- **WHEN** dispatch evaluates the acceptance result
- **THEN** the runtime does not immediately record the change as `stalled` solely due to the first denial
- **AND** existing acceptance retry, command-failure, or follow-up behavior may continue according to the non-blocker result path

#### Scenario: repeated unresolved acceptance permission denial becomes stalled

- **GIVEN** acceptance output, command error text, or FAIL findings contain a permission/policy denial
- **AND** the same denied target or equivalent denial signature was observed in a prior acceptance/apply cycle
- **AND** no repository-visible progress or changed acceptance evidence occurred between observations
- **WHEN** dispatch evaluates the repeated unresolved denial
- **THEN** the change is recorded as a non-terminal `stalled` hold
- **AND** dispatch does not append ordinary implementation follow-up tasks for that denial
- **AND** dispatch does not return to apply for that denial
- **AND** dispatch does not return terminal `error` for that denial

#### Scenario: normal acceptance failure remains retryable

- **GIVEN** acceptance FAIL findings describe implementation defects and do not match a repeated unresolved permission/policy denial
- **WHEN** dispatch handles the FAIL result
- **THEN** follow-up tasks are recorded as before
- **AND** the runtime returns to apply as before

### Requirement: Repeated Permission Blockers Avoid Cycle-Limit Degradation

When a repeated unresolved permission/policy denial is classified as a stalled execution blocker, the runtime SHALL stop routing that change through the apply/acceptance retry loop for that blocker.

#### Scenario: repeated permission denial does not become max-cycle error

- **GIVEN** the same unresolved permission/policy denial has recurred without repository-visible progress
- **WHEN** the runtime classifies it as a stalled execution blocker
- **THEN** the change is displayed as `stalled`
- **AND** the repeated blocker is not allowed to continue until `Max apply+acceptance cycles reached`
- **AND** the terminal state remains non-error so the operator can fix permissions and resume
