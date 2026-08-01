# parallel-execution Specification

## Purpose
Defines parallel change execution using jj workspaces or Git worktrees.
## Requirements

### Requirement: Shared Parallel Orchestration Service

Parallel merge retry dispatch SHALL handle deferred merge attempts as pending/deferred outcomes rather than successful completed background merge tasks. A background merge task MUST report successful completion only when the merge attempt actually integrated the change into base or determined from repository-visible evidence that the change was already integrated.

Parallel ordinary apply dispatch MUST treat reducer terminal-error state as a stop gate. After a change emits an apply, acceptance, archive, dispatch, or workspace execution error, scheduler reanalysis, queue reconciliation, and workspace resume scans MUST NOT dispatch that same change to apply again unless explicit retry intent has cleared the recoverable error terminal state. Existing workspaces MAY remain available for operator inspection or explicit retry.

Dependency analysis MUST continue to treat an errored dependency as a dispatch blocker for dependents until the dependency is explicitly retried and reaches repository-visible success.

When configured for persistent lifetime and fully drained, the scheduler MUST remain alive without timer-driven repository/worktree polling. A fully drained persistent scheduler means there is no local queued work, no in-flight workspace task, no reducer-owned resolve/reject waiter, no active manual resolve, and no pending merge or push task. In that state, the scheduler SHALL wait for explicit wake events such as dynamic queue notifications or scheduler retry notifications before running queue reconciliation, worktree scans, or base-branch merge-state checks again.

When configured for push post-archive mode, the parallel service SHALL preserve the existing apply, acceptance, and archive flow, then push the completed local change branch to the selected remote instead of merging it into the original base branch. Push mode MUST push the local branch to the same-named remote branch and MUST NOT support destination branch override syntax.

<!-- Expected canonical result after archive: `parallel-execution` will define push post-archive mode as an opt-in terminal action that substitutes remote branch push for base merge while preserving the existing pre-terminal pipeline. -->

#### Scenario: push mode skips base merge

- **GIVEN** parallel execution is running with push post-archive mode using remote `origin`
- **AND** change `alpha` has completed apply, acceptance, and archive in worktree branch `alpha`
- **WHEN** the post-archive terminal action runs
- **THEN** Conflux pushes `alpha` to remote `origin` as `alpha:alpha`
- **AND** Conflux does not checkout the original base branch to merge `alpha`
- **AND** the original base branch HEAD is not advanced by the terminal action

#### Scenario: push mode cleans up after successful push

- **GIVEN** change `alpha` is archive-complete in worktree branch `alpha`
- **AND** push mode successfully pushes `alpha:alpha` to the selected remote
- **WHEN** terminal action cleanup runs
- **THEN** the worktree for `alpha` is cleaned up through the normal safe cleanup path
- **AND** the change is reported as pushed rather than merged

#### Scenario: push failure preserves workspace

- **GIVEN** change `alpha` is archive-complete in worktree branch `alpha`
- **AND** push mode cannot push to the selected remote
- **WHEN** the push command fails
- **THEN** Conflux reports a push failure with the remote, branch, and command error context
- **AND** the worktree and local branch for `alpha` remain available for inspection or retry
- **AND** the change is not reported as merged or pushed

#### Scenario: push mode does not run on_merged hook

- **GIVEN** `hooks.on_merged` is configured
- **AND** parallel execution is running with push post-archive mode
- **WHEN** change `alpha` is successfully pushed to the remote
- **THEN** `hooks.on_merged` is not executed for `alpha`
- **AND** no `MergeCompleted` event is emitted for push success

### Requirement: Archived dependency references are explicitly classified

The system SHALL classify active proposal metadata dependency targets using repository-visible evidence that distinguishes queued, in-flight, resolving, archived, rejected, and missing targets.

Archived dependency references MUST NOT collapse into generic parse/json failures. Rejected dependency references MUST NOT collapse into generic missing dependency failures when `REJECTED.md` evidence exists.

Rejected and missing dependency targets SHALL remain fail-closed dispatch blockers. Archived dependency targets SHALL remain explicitly classified as archived, but archive evidence alone MUST NOT satisfy dependent dispatch. A dependent change whose dependency is resolving, awaiting resolve integration, or archived but not merged into the scheduler's effective dependency base MUST remain blocked until repository-visible merge evidence shows the dependency is merged into that effective dependency base. Resolve completion signaling without matching repository-visible integration evidence MUST NOT unblock the dependent. The effective dependency base SHALL be the branch or tree context Conflux uses as the accumulated integration result for dispatch decisions; in ordinary runs this MAY be the original branch, while stacked orchestration MUST use the repository-visible integration context that contains completed dependency merge/archive commits.

<!-- Expected canonical result after archive: resolving dependencies remain blocked until repository-visible integration evidence exists, while unrelated changes retain parallel dispatch eligibility. -->

#### Scenario: Resolving dependency blocks dependent dispatch

- **GIVEN** queued change `alpha` declares dependency `beta`
- **AND** `beta` is in active resolve or resolve-wait state
- **AND** the effective dependency base does not contain repository-visible merge evidence for `beta`
- **WHEN** scheduler dispatch selection evaluates `alpha`
- **THEN** `alpha` remains dependency-blocked
- **AND** apply is not started for `alpha`

#### Scenario: Resolve completion without integration evidence remains blocked

- **GIVEN** queued change `alpha` declares dependency `beta`
- **AND** the resolve command for `beta` has completed
- **AND** the effective dependency base does not yet contain repository-visible merge evidence for `beta`
- **WHEN** scheduler reanalysis evaluates `alpha`
- **THEN** `alpha` remains dependency-blocked
- **AND** resolve completion signaling alone does not satisfy the dependency

#### Scenario: Integrated resolved dependency unblocks dependent

- **GIVEN** queued change `alpha` declares dependency `beta`
- **AND** resolve integration for `beta` is complete
- **AND** the effective dependency base contains repository-visible merge evidence for `beta`
- **WHEN** resolve completion triggers scheduler reanalysis
- **THEN** `alpha` becomes eligible for dispatch if no other blockers remain

#### Scenario: Unrelated work remains parallel during resolve

- **GIVEN** change `beta` is resolving
- **AND** queued change `gamma` does not depend on `beta`
- **AND** execution capacity is available
- **WHEN** scheduler dispatch selection evaluates `gamma`
- **THEN** `gamma` remains eligible for dispatch
- **AND** the active resolve does not act as a global scheduler barrier

#### Scenario: Dependency evidence failure is fail-closed

- **GIVEN** queued change `alpha` declares dependency `beta`
- **AND** Conflux cannot determine whether `beta` is merged into the effective dependency base
- **WHEN** scheduler dispatch selection evaluates `alpha`
- **THEN** `alpha` remains dependency-blocked
- **AND** apply is not started for `alpha`

### Requirement: Dependency-blocked diagnostics are stable and non-spamming

The scheduler SHALL preserve dependency-blocked state for queued changes that cannot dispatch, but it MUST NOT repeatedly emit identical operator-visible blocked/error diagnostics while the blocked change has the same repository-visible dependency blocker signature.

A blocker signature SHALL include at least the blocked change id, dependency ids, and dependency target classes. When the signature changes, the scheduler SHALL emit a fresh diagnostic and re-evaluate dispatch using the updated dependency evidence.

Dispatch-capacity-zero diagnostics SHALL be treated as operator-visible diagnostics subject to the same stability and non-spamming rule. The signature for a capacity-zero diagnostic SHALL include at least the analysis order (or queued change ids), `queued.len()`, `in_flight.len()`, and `max_parallelism`. When any component of the signature changes, the scheduler SHALL emit a fresh diagnostic.

All operator-visible scheduler diagnostics (including but not limited to dependency-blocked, capacity-zero, no-analysis, analysis-failure, queue-reconciliation, and merge-deferred) SHALL be emitted through a single unified `DiagnosticDeduplicationStore` implementation. Each diagnostic type SHALL register its own key shape with the store; duplicate keys for the same type SHALL suppress repeated operator-visible events.

#### Scenario: Repeated identical capacity-zero state does not spam logs

- **GIVEN** the scheduler has already emitted the `dispatch_capacity_zero_after_analysis` diagnostic for a given `(order, queued.len(), in_flight.len(), max_parallelism)` signature
- **WHEN** later scheduler re-analysis loops observe the identical zero-capacity signature
- **THEN** no duplicate operator-visible `dispatch_capacity_zero_after_analysis` log is appended
- **AND** dispatch remains suppressed

#### Scenario: Changed capacity-zero signature emits a fresh diagnostic

- **GIVEN** the scheduler previously emitted `dispatch_capacity_zero_after_analysis` for a signature with `in_flight.len() == 3`
- **WHEN** an in-flight change completes and `in_flight.len()` decreases to 2 while `queued` work remains
- **THEN** the scheduler emits a fresh `dispatch_capacity_zero_after_analysis` diagnostic reflecting the updated signature
- **AND** ordinary apply dispatch remains suppressed until a positive slot count is observed

#### Scenario: All diagnostic types share a unified deduplication implementation

- **GIVEN** the scheduler emits diagnostics of any of the nine supported types
- **WHEN** the same diagnostic key is observed twice without an intervening state change
- **THEN** the second emission is suppressed by the single `DiagnosticDeduplicationStore` instance
- **AND** no per-type HashSet boilerplate remains in `ParallelExecutor`

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

Parallel and serial execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace. The acceptance loop SHALL parse stdout to determine pass/fail/continue/stalled-hold outcomes, and MUST NOT use exit code as the acceptance verdict when a canonical verdict is present.

A completed acceptance command that emits no canonical verdict MUST be classified as an explicit missing-verdict protocol failure, not as an intentional `CONTINUE`. The runtime MUST record bounded output evidence and emit an actionable operator-visible diagnostic. Missing-verdict failures MUST NOT consume or enter the configured retry path reserved for explicit canonical `CONTINUE` verdicts.

During an active run, serial and parallel execution MUST instead apply the same dedicated missing-verdict protocol-retry policy. While the dedicated budget remains, runtime MUST invoke the normal configured acceptance command again with bounded Conflux-managed prior acceptance output, attempt evidence, current workspace context, and a trusted corrective instruction requiring exactly one canonical verdict. This continuation MUST NOT depend on harness session resume, harness-specific CLI flags, provider events, or external managed-job identifiers.

The dedicated policy MUST allow no more than two retries after the initial missing-verdict attempt. Its counter MUST be independent from explicit-`CONTINUE` accounting and MUST reset after any canonical verdict. Exhaustion MUST produce a terminal missing-verdict protocol failure with bounded evidence and attempt-count diagnostics. Acceptance command launch or execution failure MUST retain command-failure routing and MUST NOT be reclassified as a missing-verdict retry.

Acceptance execution MUST NOT create a workspace-root `ACCEPTANCE_REPORT.json` artifact or another generated retry checkpoint for PASS, command failure, FAIL, CONTINUE, stalled-hold, or missing-verdict outcomes. Acceptance outcomes MAY be recorded in active-run memory, events, or non-authoritative observability logs, but archive and resume routing MUST remain derivable from workspace file/git state. After process restart, an applied but unarchived workspace MUST run acceptance again and MUST NOT infer acceptance from prior narrative output.

When acceptance returns a stalled-hold compatibility verdict for infrastructure, external dependency, missing non-mockable credential, or pending-verification evidence, parallel execution SHALL record a non-terminal stalled hold and SHALL NOT invoke terminal rejection flow solely because of that verdict.

#### Scenario: status-only exit continues through a dedicated protocol retry

- **GIVEN** an acceptance command reports that it is waiting for owned verification
- **AND** the command exits without emitting a canonical verdict
- **AND** the active run has missing-verdict retry budget remaining
- **WHEN** acceptance execution classifies the completed command
- **THEN** the result remains an explicit missing-verdict protocol failure and is not classified as `CONTINUE`
- **AND** runtime records bounded evidence and invokes the normal acceptance command again
- **AND** the new prompt includes bounded prior attempt context plus a corrective canonical-verdict instruction
- **AND** the configured explicit-`CONTINUE` counter is unchanged
- **AND** queue reconciliation does not classify the active retry as `terminal_error_retry_required`

#### Scenario: continuation is harness neutral

- **GIVEN** any configured acceptance command can receive the normal Conflux prompt
- **WHEN** runtime retries after a missing verdict
- **THEN** continuity is provided only through Conflux-managed prompt context and workspace evidence
- **AND** runtime does not require a harness session ID, resume flag, provider event, or external job ID

#### Scenario: missing-verdict retry budget is exhausted

- **GIVEN** an initial acceptance attempt and two consecutive protocol retries all emit no canonical verdict
- **WHEN** runtime classifies the third consecutive missing verdict
- **THEN** it emits the terminal missing-verdict protocol failure
- **AND** the diagnostic identifies the exhausted attempts and includes bounded evidence
- **AND** no fourth protocol retry starts

#### Scenario: canonical outcome resets protocol retry state

- **GIVEN** acceptance previously entered a missing-verdict protocol retry
- **WHEN** a later invocation emits canonical PASS, FAIL, CONTINUE, or stalled-hold output
- **THEN** that canonical outcome retains its existing routing semantics
- **AND** the consecutive missing-verdict retry count resets
- **AND** explicit `CONTINUE` retains its configured continuation policy

#### Scenario: process restart uses workspace evidence

- **GIVEN** an acceptance protocol retry was active before process termination
- **AND** the workspace remains applied but unarchived
- **WHEN** Conflux restarts without out-of-worktree runtime state
- **THEN** it runs acceptance again from workspace file/git state
- **AND** it does not infer PASS from prior missing-verdict output
- **AND** it does not require a generated acceptance retry checkpoint

#### Scenario: missing verdict does not create report artifact

- **GIVEN** an acceptance command exits without a canonical verdict
- **WHEN** runtime records or retries the missing-verdict failure
- **THEN** the workspace root does not contain `ACCEPTANCE_REPORT.json` or another generated acceptance retry checkpoint
- **AND** evidence is carried through existing prompt, history, event, and observability paths

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

明示的な queue notification により dynamic queue ingestion または reducer reconciliation から新しい loadable queued candidate が scheduler-local queued work に追加された場合、scheduler はその追加を debounce 対象の timer/poll 再確認として扱ってはならない（MUST NOT）。この場合、現在の scheduler iteration が初回でなく、queue debounce timestamp が新しい場合でも、dependency analysis を開始しなければならない（MUST）。

ただし、同一状態で候補追加を伴わない queue wake、timer wake、blocked-only drain、または candidate-unavailable 状態は、既存の debounce / diagnostic dedupe / notification-driven idle policy に従ってよい（MAY）。

Scheduler reconciliation は reducer-visible queued work が analysis 対象へ取り込まれない理由を観測可能にしなければならない（SHALL）。ただし、同じ change と同じ理由が scheduler loop ごとに連続する場合、user-visible logs と WARN-level debug log entries への出力は dedupe、rate-limit、または summary 化されなければならない（SHALL）。

Reducer-visible queued reconciliation MUST NOT refresh an existing queue debounce timestamp merely because the same reducer-visible queued intent is reconstructed again from repository-visible OpenSpec state. Reconciliation MAY initialize the debounce timestamp when reducer-visible queued work is first reconstructed and no timestamp exists, but repeated rediscovery of the same reducer-owned queued state MUST allow the original debounce window to elapse or must be handled by the existing explicit queue-notification bypass rules.

<!-- Expected canonical result after archive: `Parallel Analysis Targeting` prevents reducer-visible queued reconciliation from starving analysis by repeatedly resetting queue debounce, while preserving explicit queue-addition bypass and blocked-only idle behavior. -->

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

#### Scenario: explicit TUI queue addition bypasses queue debounce

- **GIVEN** parallel execution is already running beyond the first scheduler analysis iteration
- **AND** the queue debounce timestamp is fresh enough that timer-driven reanalysis would normally be deferred
- **WHEN** the operator presses `x` in the TUI Changes view and a `not queued` loadable change is added to scheduler-local queued work through dynamic queue ingestion
- **THEN** dependency analysis starts for the queued candidate without waiting for the debounce period to expire
- **AND** the analysis target set includes queued candidates only

#### Scenario: reducer-visible queue reconciliation bypasses debounce when it adds loadable work

- **GIVEN** reducer-visible queued intent exists for change `gamma`
- **AND** `gamma` is loadable from active OpenSpec changes
- **AND** scheduler-local queued work does not yet contain `gamma`
- **AND** the queue debounce timestamp is fresh enough that timer-driven reanalysis would normally be deferred
- **WHEN** scheduler reconciliation adds `gamma` to scheduler-local queued work
- **THEN** dependency analysis starts for `gamma` without waiting for the debounce period to expire

#### Scenario: repeated reducer-visible reconciliation does not starve debounce

- **GIVEN** reducer-visible queued intent exists for change `epsilon`
- **AND** `epsilon` is loadable from active OpenSpec changes
- **AND** queue debounce timestamp is already set from a prior queue addition
- **WHEN** scheduler reconciliation reconstructs `epsilon` again on repeated scheduler ticks without a new explicit queue edit
- **THEN** the existing queue debounce timestamp is not refreshed solely by that repeated reconciliation
- **AND** the original debounce window can elapse so dependency analysis can run normally

#### Scenario: zero capacity still analyzes explicit queue additions without dispatching

- **GIVEN** all ordinary dispatch slots are occupied or held by resolve/manual work
- **AND** a loadable change `delta` is explicitly added to scheduler-local queued work by dynamic queue ingestion or reducer reconciliation
- **WHEN** the scheduler evaluates the queue notification
- **THEN** dependency analysis starts for `delta`
- **AND** ordinary apply dispatch is suppressed until execution capacity becomes available
- **AND** the suppression is observable through a capacity-gated diagnostic or equivalent event

#### Scenario: queue wake without new candidate may remain debounceable

- **GIVEN** a queue notification wakes the scheduler
- **AND** dynamic queue ingestion and reducer reconciliation do not add any new loadable queued candidate
- **WHEN** the scheduler evaluates reanalysis eligibility
- **THEN** the scheduler may defer analysis according to existing debounce, blocked-only, or notification-driven idle policy

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

The system SHALL send completion events and messages only when processing completes normally, not when stopped or cancelled by the user. The system SHALL distinguish successful completion, completion with errors, graceful stop, active-execution force stop, and scheduler-only cancellation. Operator cancellation MUST NOT be represented as an agent-command failure.

#### Scenario: Graceful stop during parallel execution should not show success message

**Given** the orchestrator is running in parallel mode
**And** at least one change is queued for processing
**When** the user triggers graceful stop before processing completes
**Then** the orchestrator stops processing after the graceful boundary
**And** sends `OrchestratorEvent::Stopped`
**And** does not send `OrchestratorEvent::AllCompleted`
**And** displays `Processing stopped` without a success completion message

#### Scenario: Force stop of active execution remains cancellation rather than failure

**Given** the orchestrator is running in parallel mode
**And** an agent command or in-flight execution is active
**When** the user triggers force stop
**Then** active execution cancellation and managed process cleanup are requested
**And** the outcome is classified as stopped or cancelled
**And** the system does not display `Execution failed: Agent command failed`
**And** the system does not display `Processing completed with errors`
**And** the system does not send `OrchestratorEvent::AllCompleted`

#### Scenario: Scheduler-only stop does not claim forceful process termination

**Given** the parallel scheduler remains alive in `MergeWait`, `ResolveWait`, deferred merge, or idle waiting
**And** no agent command or in-flight execution is active
**When** the user requests immediate stop
**Then** the scheduler/orchestrator is cancelled
**And** `Processing stopped` is displayed once
**And** no force-stop, process-termination, execution-failure, or normal-completion message is displayed
**And** `OrchestratorEvent::AllCompleted` is not sent

#### Scenario: Successful parallel execution completion shows success message

**Given** the orchestrator is running in parallel mode
**And** multiple changes are queued for processing
**When** all changes complete successfully without errors or cancellation
**Then** the orchestrator sends `OrchestratorEvent::AllCompleted`
**And** displays the existing successful completion messages

#### Scenario: Parallel execution with genuine errors shows warning message

**Given** the orchestrator is running in parallel mode
**When** a non-cancellation execution error occurs
**And** all eligible queued work has been attempted
**Then** the orchestrator sends `OrchestratorEvent::AllCompleted`
**And** displays `Processing completed with errors`
**And** does not display a successful completion message

### Requirement: Loop termination reason must be tracked and distinguished

The system SHALL track the reason for loop termination as normal completion, genuine execution error, graceful stop, active-execution force stop, scheduler-only cancellation, or merge wait. This termination reason SHALL control terminal logs and events without inferring process activity from TUI mode or error-message text. Operator cancellation SHALL request cancellation without dropping the running scheduler future and SHALL establish terminal stop only after the scheduler reaches its bounded cleanup barrier, including active task drain and pending background merge/base-lane result handling.

#### Scenario: Operator cancellation reaches terminal classification

**Given** the global parallel cancellation token is triggered by an operator stop
**When** the outer parallel orchestration boundary observes cancellation before the scheduler future returns
**Then** the termination reason is recorded as stopped or cancelled
**And** cancellation is not converted to `OrchestratorError::AgentCommand`
**And** the outer boundary continues polling the scheduler future until its bounded cleanup barrier completes
**And** active task drain, registered execution-handle cleanup, and pending merge/base-lane result handling precede terminal stop
**And** a cleanup deadline or managed escalation remains classified as operator cancellation rather than execution failure
**And** later terminal event handling remains idempotent if the frontend already applied `OrchestratorEvent::Stopped`
**And** `Processing stopped` is not logged more than once

#### Scenario: Genuine failure remains distinct

**Given** a parallel service or command fails without operator cancellation
**When** the outer parallel orchestration boundary handles the result
**Then** the termination reason is recorded as genuine execution error
**And** existing failure and completion-with-errors reporting remains enabled

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

利用可能スロットが 0 の場合でも、queued に ordinary dispatchable candidate が存在するなら、システムは queue classification、reducer-visible queued intent reconciliation、dependency analysis、operator-visible diagnostics を実行できなければならない（MUST）。ただし ordinary apply dispatch は、dispatch 直前に再計算した利用可能スロットが 1 以上になるまで開始してはならない（MUST NOT）。

resolve、workspace、merge completion、repair candidate addition、または slot recovery による即時 re-analysis trigger は、対応する state-transition event ごとに一度だけ利用されなければならない（MUST）。scheduler は queued work に対する re-analysis / dispatch evaluation をその trigger で実際に評価した後にのみ trigger を消費しなければならず（MUST）、evaluation を実行しなかった loop で未評価の trigger を破棄してはならない（MUST NOT）。

一度消費した completion、repair candidate、または slot recovery trigger は、明示的な新しい state-transition event がない timer wake で再利用されてはならない（MUST NOT）。timer wake は有限時間 queue debounce policy に従わなければならず（MUST）、debounce 経過後も同一の completed analysis input を変更なしに反復実行してはならない（MUST NOT）。

scheduler は ordinary timer-driven dependency analysis の直前に、queued change の analysis 入力、in-flight membership、利用可能 capacity、および repository-visible effective dependency-base evidence を表す deterministic runtime signature を評価しなければならない（MUST）。同じ signature に対する usable analysis result が active process 内ですでに完了している場合、明示的な新しい queue addition、completion、repair candidate、または slot recovery event を伴わない timer wake は高価な dependency analyzer を再実行してはならない（MUST NOT）。

signature は同一 change ID の proposal dependency、prompt-relevant metadata、または analyzer が読む proposal content の変更を識別できなければならず（MUST）、queued ID と件数だけで構成してはならない（MUST NOT）。queued と in-flight の双方について prompt が参照する proposal file content を含めなければならない（MUST）。effective dependency-base revision は dependency classification が merge evidence を評価する同じ selected branch/ref から解決されなければならず（MUST）、その ref revision が変化した場合は current checkout commit が不変でも signature は変化しなければならない（MUST）。

signature 構築に必要な proposal read または effective-base revision resolution が失敗した場合、scheduler は fail-open で dependency analysis を許可し、signature を記録せず、loop を終了してはならない（MUST）。ただし ordinary timer による再試行は既存の 10 秒 queue debounce cadence より頻繁に実行してはならず（MUST NOT）、失敗が継続する間の 500 ms timer wake ごとに proposal/VCS probe または dependency analyzer を起動してはならない（MUST NOT）。新しい明示的 edge trigger はこの失敗再試行 deadline を event ごとに一度 bypass してよい（MAY）。

queue addition、completion、repair candidate、および slot recovery の明示的 edge trigger は、matching signature が存在しても event ごとに一度の即時 analysis を許可しなければならない（MUST）。scheduler は analyzer result provenance を runtime 内で識別しなければならない（MUST）。healthy LLM result または意図的な metadata-only result は non-expiring completed signature を記録してよい。recoverable LLM failure による metadata fallback は degraded signature として記録し、記録から5分後の最初の eligible timer wake で unchanged input に対する一度の retry を許可しなければならない（MUST）。直前の repository probe deadline はこの degraded expiry を越えて retry を遅延させてはならない（MUST NOT）。

usable result を生成しない analyzer path は completed signature を記録してはならない（MUST NOT）。reducer-visible queued work が残る場合、その unusable result だけを理由に scheduler loop を終了してはならず（MUST NOT）、次の debounce-eligible timer evaluation または明示的 edge による retry を許可しなければならない（MUST）。

analysis 後も available capacity が正で、in-flight work が空であり、selected dispatch が 0 件である場合、scheduler はその result による suppression を記録してはならない（MUST NOT）。次の debounce-eligible timer evaluation は同じ input を再分析できなければならない（MUST）。

manual resolve、automatic resolve、workspace task、background merge、deferred retry、または failure / early-return path によって利用可能スロットが回復する場合、scheduler は explicit wake event、slot recovery detection、または現在 signature の変化を検出する有限時間 timer evaluation により queued work を再評価しなければならない（MUST）。sticky な過去 trigger または unchanged completed signature の反復利用だけを capacity-recovery liveness の根拠としてはならない（MUST NOT）。

analysis signature、completed record、および失敗再試行 deadline は active scheduler process の memory 内だけに保持しなければならず（MUST）、workflow next action、acceptance、archive、merge eligibility を決定する durable out-of-worktree state として保存または再利用してはならない（MUST NOT）。process restart 後の初回 eligible evaluation は以前の log、diagnostic cache、analysis signature、または retry deadline により抑止されてはならない（MUST NOT）。

スケジューラは reducer-visible queued work が存在するのに re-analysis または dispatch を開始しない場合、その理由を観測可能なログまたはイベントとして出力しなければならない（SHALL）。unchanged completed analysis input または bounded signature-failure retry による analyzer suppression は deduplicated operator-visible reason として識別可能でなければならない（SHALL）。

TUI は distinct な re-analysis attempt を operator-visible に表示しなければならない（SHALL）。同じ attempt の重複 delivery は抑止してよいが、`remaining_changes` が同じという理由だけで別 attempt の analysis-started 表示を抑止してはならない（MUST NOT）。analyzer invocation 前に suppression された timer evaluation は distinct analysis attempt として表示してはならない（MUST NOT）。

<!-- Expected canonical result after archive: ordinary timer suppression uses the same effective-base ref as dependency classification, unusable results cannot terminate queued work, and fail-open signature failures retry at a bounded cadence without restoring the 500 ms analyzer loop. -->

#### Scenario: effective dependency base ref change invalidates suppression

- **GIVEN** queued change と in-flight membership、capacity、proposal content、および current checkout commit は変化していない
- **AND** dependent change は selected effective dependency-base ref 上の integration evidence を待っている
- **WHEN** selected effective dependency-base ref の revision だけが前進する
- **THEN** current analysis input signature は previous completed signature と異なる
- **AND** scheduler は bounded timer evaluation により dependency analysis と dispatch eligibility を再評価する

#### Scenario: unusable empty analysis keeps queued scheduler alive

- **GIVEN** reducer-visible queued work が存在する
- **AND** in-flight work は空である
- **WHEN** dependency analyzer が usable order を生成せず終了する
- **THEN** scheduler は completed signature を記録しない
- **AND** scheduler loop はその result だけを理由に終了しない
- **AND** 次の debounce-eligible timer evaluation または明示的 edge は dependency analysis を再試行できる

#### Scenario: persistent signature failure is fail-open but rate-limited

- **GIVEN** proposal read または effective-base revision resolution が継続して失敗する
- **WHEN** ordinary 500 ms timer wake が10秒未満の間隔で繰り返される
- **THEN** scheduler は failed input をcompleted signatureとして記録しない
- **AND** scheduler loop は継続する
- **AND** ordinary timer による proposal/VCS probe と dependency analyzer invocation は10秒に一度を超えない
- **AND** deadline 後の最初の eligible evaluation は signature construction と dependency analysis を再試行する

#### Scenario: explicit edge bypasses signature failure retry deadline once

- **GIVEN** signature construction failure 後の ordinary retry deadline が未到達である
- **WHEN** new queue addition、completion、repair candidate、または slot recovery event が発生する
- **THEN** scheduler はその event に対して一度だけ即時 evaluation を許可する
- **AND** failure が継続する場合、event 消費後の ordinary timer wake は bounded retry cadence に戻る

#### Scenario: degraded expiry is not delayed by repository probe cadence

- **GIVEN** recoverable-failure metadata fallback の degraded signature が記録されている
- **AND** 5分expiry直前の repository probe が同じ signature を確認した
- **WHEN** degraded record の記録から5分が経過する
- **THEN** scheduler は最初の eligible timer wake で unchanged input に対する一度の retry を許可する
- **AND** 直前に設定された10秒 probe deadlineはretryをさらに遅延させない

#### Scenario: completed signature suppresses immediate timer I/O

- **GIVEN** scheduler は healthy usable analysis result と captured signature を記録した
- **WHEN** 記録から10秒未満の間に500 ms timer wakeが発生する
- **THEN** scheduler は dependency analyzerを再実行しない
- **AND** proposal fingerprintまたはVCS revisionを再取得しない

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

実行中の change が存在せず、queued の change も空の場合、オーケストレーションは完了状態にならなければならない（MUST）。ただし reducer-visible queued intent が存在する場合、その intent が terminal / active / missing などの理由で analysis 対象外であることが確認されるまで完了状態として扱ってはならない（MUST NOT）。

queued に含まれない change（例: merged 済み change、実行済み change、削除済み change）は analysis 対象から除外されなければならない（MUST）。

Archived-dirty repair candidate は workspace-derived repair trigger として扱われなければならない（MUST）。scheduler は同じ unchanged archived-dirty repair candidate の再発見を通常の user/reducer queue addition と同じ debounce 更新として扱ってはならない（MUST NOT）。

Reducer-terminal final states such as `merged`, `archived`, and `rejected` MUST be dispatch stop gates for ordinary apply/acceptance/archive work. A stale dynamic queue entry, stale scheduler-local candidate, or reducer reconciliation pass MUST NOT add a final terminal change to scheduler-local queued work or analysis candidates.

Recoverable terminal `error` remains distinct: it MUST NOT be dispatched through ordinary apply/acceptance/archive work unless explicit retry intent clears the terminal error according to reducer rules.

<!-- Expected canonical result after archive: `parallel-execution` will require scheduler queue ingestion, reconciliation, and dispatch selection to treat reducer-terminal final states as ordinary dispatch stop gates while preserving explicit retry semantics for terminal errors. -->

#### Scenario: terminal merged dynamic queue entry is ignored

- **GIVEN** change `alpha` is reducer-terminal `merged`
- **AND** a stale dynamic queue entry for `alpha` is popped
- **WHEN** scheduler dynamic queue ingestion evaluates `alpha`
- **THEN** `alpha` is not added to scheduler-local `queued`
- **AND** `alpha` is not included in dependency analysis candidates
- **AND** apply, acceptance, and archive are not started for `alpha`

#### Scenario: terminal merged dispatch preflight stops archive path

- **GIVEN** change `alpha` is reducer-terminal `merged`
- **AND** stale scheduler-local state attempts to dispatch `alpha`
- **WHEN** `dispatch_change_to_workspace` evaluates preflight guards
- **THEN** dispatch is skipped before workspace acquisition or reuse
- **AND** `execute_archive_in_workspace` is not called for `alpha`
- **AND** no `ArchiveStarted` event is emitted for `alpha`

#### Scenario: terminal error remains explicit retry only

- **GIVEN** change `beta` is reducer-terminal `error`
- **WHEN** ordinary scheduler dispatch evaluates `beta` without explicit retry intent
- **THEN** `beta` is skipped as retry-required
- **AND** apply, acceptance, and archive are not started for `beta`
- **WHEN** explicit retry intent clears the terminal error
- **THEN** `beta` can become eligible for ordinary queued dispatch again

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

Parallel and serial execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace. The acceptance loop SHALL parse stdout to determine pass/fail/continue/stalled-hold outcomes, and MUST NOT use exit code as the acceptance verdict when a canonical verdict is present.

A completed acceptance command that emits no canonical verdict MUST be classified as an explicit missing-verdict protocol failure, not as an intentional `CONTINUE`. The runtime MUST record bounded output evidence and emit an actionable operator-visible diagnostic. Missing-verdict failures MUST NOT consume or enter the configured retry path reserved for explicit canonical `CONTINUE` verdicts.

During an active run, serial and parallel execution MUST instead apply the same dedicated missing-verdict protocol-retry policy. While the dedicated budget remains, runtime MUST invoke the normal configured acceptance command again with bounded Conflux-managed prior acceptance output, attempt evidence, current workspace context, and a trusted corrective instruction requiring exactly one canonical verdict. This continuation MUST NOT depend on harness session resume, harness-specific CLI flags, provider events, or external managed-job identifiers.

The dedicated policy MUST allow no more than two retries after the initial missing-verdict attempt. Its counter MUST be independent from explicit-`CONTINUE` accounting and MUST reset after any canonical verdict. Exhaustion MUST produce a terminal missing-verdict protocol failure with bounded evidence and attempt-count diagnostics. Acceptance command launch or execution failure MUST retain command-failure routing and MUST NOT be reclassified as a missing-verdict retry.

Acceptance execution MUST NOT create a workspace-root `ACCEPTANCE_REPORT.json` artifact or another generated retry checkpoint for PASS, command failure, FAIL, CONTINUE, stalled-hold, or missing-verdict outcomes. Acceptance outcomes MAY be recorded in active-run memory, events, or non-authoritative observability logs, but archive and resume routing MUST remain derivable from workspace file/git state. After process restart, an applied but unarchived workspace MUST run acceptance again and MUST NOT infer acceptance from prior narrative output.

When acceptance returns a stalled-hold compatibility verdict for infrastructure, external dependency, missing non-mockable credential, or pending-verification evidence, parallel execution SHALL record a non-terminal stalled hold and SHALL NOT invoke terminal rejection flow solely because of that verdict.

#### Scenario: status-only exit continues through a dedicated protocol retry

- **GIVEN** an acceptance command reports that it is waiting for owned verification
- **AND** the command exits without emitting a canonical verdict
- **AND** the active run has missing-verdict retry budget remaining
- **WHEN** acceptance execution classifies the completed command
- **THEN** the result remains an explicit missing-verdict protocol failure and is not classified as `CONTINUE`
- **AND** runtime records bounded evidence and invokes the normal acceptance command again
- **AND** the new prompt includes bounded prior attempt context plus a corrective canonical-verdict instruction
- **AND** the configured explicit-`CONTINUE` counter is unchanged
- **AND** queue reconciliation does not classify the active retry as `terminal_error_retry_required`

#### Scenario: continuation is harness neutral

- **GIVEN** any configured acceptance command can receive the normal Conflux prompt
- **WHEN** runtime retries after a missing verdict
- **THEN** continuity is provided only through Conflux-managed prompt context and workspace evidence
- **AND** runtime does not require a harness session ID, resume flag, provider event, or external job ID

#### Scenario: missing-verdict retry budget is exhausted

- **GIVEN** an initial acceptance attempt and two consecutive protocol retries all emit no canonical verdict
- **WHEN** runtime classifies the third consecutive missing verdict
- **THEN** it emits the terminal missing-verdict protocol failure
- **AND** the diagnostic identifies the exhausted attempts and includes bounded evidence
- **AND** no fourth protocol retry starts

#### Scenario: canonical outcome resets protocol retry state

- **GIVEN** acceptance previously entered a missing-verdict protocol retry
- **WHEN** a later invocation emits canonical PASS, FAIL, CONTINUE, or stalled-hold output
- **THEN** that canonical outcome retains its existing routing semantics
- **AND** the consecutive missing-verdict retry count resets
- **AND** explicit `CONTINUE` retains its configured continuation policy

#### Scenario: process restart uses workspace evidence

- **GIVEN** an acceptance protocol retry was active before process termination
- **AND** the workspace remains applied but unarchived
- **WHEN** Conflux restarts without out-of-worktree runtime state
- **THEN** it runs acceptance again from workspace file/git state
- **AND** it does not infer PASS from prior missing-verdict output
- **AND** it does not require a generated acceptance retry checkpoint

#### Scenario: missing verdict does not create report artifact

- **GIVEN** an acceptance command exits without a canonical verdict
- **WHEN** runtime records or retries the missing-verdict failure
- **THEN** the workspace root does not contain `ACCEPTANCE_REPORT.json` or another generated acceptance retry checkpoint
- **AND** evidence is carried through existing prompt, history, event, and observability paths

### Requirement: Shared Parallel Orchestration Service

Parallel merge retry dispatch SHALL handle deferred merge attempts as pending/deferred outcomes rather than successful completed background merge tasks. A background merge task MUST report successful completion only when the merge attempt actually integrated the change into base or determined from repository-visible evidence that the change was already integrated.

Parallel ordinary apply dispatch MUST treat reducer terminal-error state as a stop gate. After a change emits an apply, acceptance, archive, dispatch, or workspace execution error, scheduler reanalysis, queue reconciliation, and workspace resume scans MUST NOT dispatch that same change to apply again unless explicit retry intent has cleared the recoverable error terminal state. Existing workspaces MAY remain available for operator inspection or explicit retry.

Dependency analysis MUST continue to treat an errored dependency as a dispatch blocker for dependents until the dependency is explicitly retried and reaches repository-visible success.

When configured for persistent lifetime and fully drained, the scheduler MUST remain alive without timer-driven repository/worktree polling. A fully drained persistent scheduler means there is no local queued work, no in-flight workspace task, no reducer-owned resolve/reject waiter, no active manual resolve, and no pending merge or push task. In that state, the scheduler SHALL wait for explicit wake events such as dynamic queue notifications or scheduler retry notifications before running queue reconciliation, worktree scans, or base-branch merge-state checks again.

When configured for push post-archive mode, the parallel service SHALL preserve the existing apply, acceptance, and archive flow, then push the completed local change branch to the selected remote instead of merging it into the original base branch. Push mode MUST push the local branch to the same-named remote branch and MUST NOT support destination branch override syntax.

<!-- Expected canonical result after archive: `parallel-execution` will define push post-archive mode as an opt-in terminal action that substitutes remote branch push for base merge while preserving the existing pre-terminal pipeline. -->

#### Scenario: push mode skips base merge

- **GIVEN** parallel execution is running with push post-archive mode using remote `origin`
- **AND** change `alpha` has completed apply, acceptance, and archive in worktree branch `alpha`
- **WHEN** the post-archive terminal action runs
- **THEN** Conflux pushes `alpha` to remote `origin` as `alpha:alpha`
- **AND** Conflux does not checkout the original base branch to merge `alpha`
- **AND** the original base branch HEAD is not advanced by the terminal action

#### Scenario: push mode cleans up after successful push

- **GIVEN** change `alpha` is archive-complete in worktree branch `alpha`
- **AND** push mode successfully pushes `alpha:alpha` to the selected remote
- **WHEN** terminal action cleanup runs
- **THEN** the worktree for `alpha` is cleaned up through the normal safe cleanup path
- **AND** the change is reported as pushed rather than merged

#### Scenario: push failure preserves workspace

- **GIVEN** change `alpha` is archive-complete in worktree branch `alpha`
- **AND** push mode cannot push to the selected remote
- **WHEN** the push command fails
- **THEN** Conflux reports a push failure with the remote, branch, and command error context
- **AND** the worktree and local branch for `alpha` remain available for inspection or retry
- **AND** the change is not reported as merged or pushed

#### Scenario: push mode does not run on_merged hook

- **GIVEN** `hooks.on_merged` is configured
- **AND** parallel execution is running with push post-archive mode
- **WHEN** change `alpha` is successfully pushed to the remote
- **THEN** `hooks.on_merged` is not executed for `alpha`
- **AND** no `MergeCompleted` event is emitted for push success

### Requirement: Non-blocking Merge in Scheduler Loop

パラレルスケジューラの `tokio::select!` イベントループは、workspace 完了後の merge + コンフリクト解決処理によってブロックされてはならない（MUST NOT）。merge + resolve 処理はバックグラウンドタスクとして非同期に実行し、スケジューラループは queued change の dispatch を継続しなければならない（SHALL）。

この非ブロッキング要件は post-archive merge に限らず、すべての base-mutating lane 作業に適用されなければならない（MUST）。具体的には、ResolveWait の deferred merge retry（コンフリクト解決エージェント実行を含む）、RejectWait の rejection-review retry、および手動 resolve（TUI `M` キー由来の reducer ResolveWait promotion）の実行を、スケジューラループタスク内で直接 await してはならない（MUST NOT）。スケジューラループが行ってよいのは promotion（reducer の base-mutating lane への昇格）とバックグラウンドタスクの spawn、および結果の受信処理のみである（MUST）。

スケジューラループタスクは global merge lock の取得を待ってブロックしてはならない（MUST NOT）。merge 試行は resolve アクティブ判定をロック取得より前に評価し、ロックが取得できない場合は自動再開可能な Deferred として返却しなければならない（MUST）。Deferred は既存の merge/resolve 完了トリガで自動的に再評価されなければならない（MUST）。

merge/resolve の結果（成功・Deferred・失敗）はスケジューラループに非同期に通知され、適切に処理されなければならない（MUST）。base-mutating lane の単一占有（同時に最大1つの resolve または rejection review）は reducer の lane 占有状態によって維持されなければならない（MUST）。spawn された retry の実行中は、スケジューラはドレイン完了・persistent idle・終了判定においてその作業を未完了として扱わなければならない（MUST）。

spawn された base-mutating lane retry の結果が Merged 以外（自動再開可能な Deferred、または失敗）である場合、スケジューラは結果受信処理において reducer の base-mutating lane 占有を解放しなければならない（MUST）。自動再開可能な Deferred で終わった change は、promotion 元の wait 種別（ResolveWait / RejectWait）に復元され、以降の merge/resolve 完了トリガまたは queue notification で再 promote 可能でなければならない（MUST）。retry の失敗が `ResolveFailed` / `RejectionReviewFailed` などの失敗イベントを伴わずに終了した場合（例: workspace 喪失）も、lane 占有を解放し、運用者可視のイベントを発行しなければならない（MUST）。lane 占有の解放漏れにより promotion が恒久的に不能となる状態（生存するタスクを伴わない Resolving / Rejecting の残留）を生じさせてはならない（MUST NOT）。retry の失敗は運用者に対して 1 回だけ報告されなければならず（MUST）、retry 本体が発行した失敗イベントに加えて汎用エラーを重複報告してはならない（MUST NOT）。

spawn された retry が実マージを行わずに retry 意図を放棄して終了する場合（give-up: workspace 喪失、stale workspace path、base への既マージ検出による stale intent cleanup を含む）、retry 本体は intent 解除と同時に reducer の lane 占有を同期的に解放しなければならない（MUST）。give-up による解放では、対象 change を ResolveWait / RejectWait のいずれの wait queue にも再登録してはならない（MUST NOT）。give-up の結果が Merged 相当のトリガとしてスケジューラに届いた後、後続の ResolveWait / RejectWait waiter の promotion が可能でなければならない（MUST）。give-up 解放は terminal 遷移済みエントリおよび lane 非占有エントリに対しては no-op でなければならない（MUST）。

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

#### Scenario: Deferred merge retry resolve runs off the scheduler loop

- **GIVEN** Change A が ResolveWait であり、その deferred merge retry がコンフリクト解決エージェントの実行を必要とする
- **WHEN** スケジューラが ResolveWait retry を dispatch する
- **THEN** retry の merge + resolve 実行はバックグラウンドタスクとして spawn される
- **AND** スケジューラループは次の iteration に進み、dynamic queue 取り込み・queue reconciliation・re-analysis を継続する
- **AND** resolve エージェントの実行完了をスケジューラループタスク内で直接 await しない

#### Scenario: Change queued via dynamic queue during active resolve is analyzed promptly

- **GIVEN** Change A の resolve（手動 resolve または deferred merge retry の resolve）が進行中である
- **AND** ユーザーが TUI の `x` キーで Change B を queue に追加する
- **WHEN** スケジューラループが次の iteration を実行する
- **THEN** Change B は Change A の resolve 完了を待たずに scheduler queue へ取り込まれる
- **AND** 通常の debounce 範囲内で Change B の dependency analysis が開始される
- **AND** 再計算した利用可能スロットが 1 以上であれば Change B の apply dispatch が開始される

#### Scenario: Scheduler loop does not park on global merge lock

- **GIVEN** spawn された merge/resolve タスクが global merge lock を保持して resolve エージェントを実行中である
- **AND** ResolveWait または RejectWait の change が存在する
- **WHEN** queue notification により ResolveWait retry dispatch が評価される
- **THEN** スケジューラループタスクは global merge lock の解放を待ってブロックしない
- **AND** merge 試行はロック競合時に自動再開可能な Deferred を返す
- **AND** スケジューラループは re-analysis と diagnostics を継続できる

#### Scenario: Consecutive resolve waiters do not starve analysis

- **GIVEN** ResolveWait の change が複数存在し、それぞれの retry がコンフリクト解決を必要とする
- **AND** queued に ordinary dispatchable な Change C が存在する
- **WHEN** 先行する retry が完了して次の waiter が promote される
- **THEN** 次の retry もバックグラウンドタスクとして実行される
- **AND** Change C の re-analysis は retry の合間または実行中に行われ、retry 連鎖によって無期限に遅延しない

#### Scenario: Scheduler does not exit while spawned retry is in flight

- **GIVEN** spawn された base-mutating lane retry が実行中である
- **AND** queued と in-flight がともに空である
- **WHEN** スケジューラがドレイン完了・終了判定を評価する
- **THEN** スケジューラは終了せず retry の結果通知を待つ
- **AND** 結果受信後に ResolveWait 解消・次 waiter promotion・re-analysis が行われる

#### Scenario: Auto-resumable deferred retry releases the base-mutating lane

- **GIVEN** Change B が ResolveWait から promote され、spawn された retry の merge 試行が global merge lock 競合により自動再開可能な Deferred（"Merge lane busy"）で終了する
- **WHEN** スケジューラが retry の Deferred 結果を受信処理する
- **THEN** reducer の base-mutating lane 占有が解放される（Change B の activity が Resolving のまま残留しない）
- **AND** Change B は ResolveWait に復元され、resolve wait queue に重複なく再登録される
- **AND** 後続の merge/resolve 完了トリガまたは queue notification で Change B が再 promote される

#### Scenario: Deferred retry converges after the merge lock is released

- **GIVEN** Change B の retry が "Merge lane busy" の自動再開可能 Deferred で終了し、ResolveWait に復元されている
- **AND** global merge lock を保持していたタスクが完了して Merged 結果がスケジューラに届く
- **WHEN** スケジューラが Merged 結果の受信処理で次の waiter を dispatch する
- **THEN** Change B が promote され retry が再実行される
- **AND** ユーザー操作なしで Change B の merge が完了に到達する

#### Scenario: Retry failure without a failure event still releases the lane

- **GIVEN** Change B が ResolveWait から promote され、spawn された retry が `ResolveFailed` 等の失敗イベントを発行せずに失敗する（例: workspace が見つからない）
- **WHEN** スケジューラが retry の失敗結果を受信処理する
- **THEN** reducer の base-mutating lane 占有が解放される
- **AND** 運用者可視のイベントが 1 回発行される
- **AND** 後続の ResolveWait / RejectWait waiter の promotion が引き続き可能である

#### Scenario: Retry give-up without a merge releases the lane without re-enqueueing

- **GIVEN** Change B が ResolveWait または RejectWait から promote され、spawn された retry が workspace 喪失・stale workspace path・base への既マージ検出のいずれかにより実マージを行わず retry 意図を放棄して Merged 相当の結果を返す
- **WHEN** retry 本体が intent を解除して give-up を確定する
- **THEN** reducer の base-mutating lane 占有が同期的に解放される（Change B の activity が Resolving / Rejecting のまま残留しない）
- **AND** Change B は resolve wait queue / reject wait queue のいずれにも再登録されない
- **AND** give-up 結果の受信処理を契機として、後続の ResolveWait / RejectWait waiter が promote 可能である

#### Scenario: Give-up by the lane occupant unblocks the next waiter

- **GIVEN** Change B と Change C がともに ResolveWait に存在し、Change B が promote されている
- **AND** Change B の workspace が失われており、spawn された retry が give-up する
- **WHEN** give-up の Merged 相当結果がスケジューラの結果受信処理に届く
- **THEN** Change C が promote され、その retry がバックグラウンドタスクとして spawn される
- **AND** Change B は wait queue に存在せず、再 promote されない

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

The system SHALL send completion events and messages only when processing completes normally, not when stopped or cancelled by the user. The system SHALL distinguish successful completion, completion with errors, graceful stop, active-execution force stop, and scheduler-only cancellation. Operator cancellation MUST NOT be represented as an agent-command failure.

The parallel execution subsystem SHALL NOT run a merge stall monitor based on historical base-branch merge commit timestamps. Queue execution MUST NOT be interrupted or annotated by a monitor that does not observe current queue or scheduler progress.

#### Scenario: Graceful stop during parallel execution should not show success message

**Given** the orchestrator is running in parallel mode
**And** at least one change is queued for processing
**When** the user triggers graceful stop before processing completes
**Then** the orchestrator stops processing after the graceful boundary
**And** sends `OrchestratorEvent::Stopped`
**And** does not send `OrchestratorEvent::AllCompleted`
**And** displays `Processing stopped` without a success completion message

#### Scenario: Force stop of active execution remains cancellation rather than failure

**Given** the orchestrator is running in parallel mode
**And** an agent command or in-flight execution is active
**When** the user triggers force stop
**Then** active execution cancellation and managed process cleanup are requested
**And** the outcome is classified as stopped or cancelled
**And** the system does not display `Execution failed: Agent command failed`
**And** the system does not display `Processing completed with errors`
**And** the system does not send `OrchestratorEvent::AllCompleted`

#### Scenario: Scheduler-only stop does not claim forceful process termination

**Given** the parallel scheduler remains alive in `MergeWait`, `ResolveWait`, deferred merge, or idle waiting
**And** no agent command or in-flight execution is active
**When** the user requests immediate stop
**Then** the scheduler/orchestrator is cancelled
**And** `Processing stopped` is displayed once
**And** no force-stop, process-termination, execution-failure, or normal-completion message is displayed
**And** `OrchestratorEvent::AllCompleted` is not sent

#### Scenario: Successful parallel execution completion shows success message

**Given** the orchestrator is running in parallel mode
**And** multiple changes are queued for processing
**When** all changes complete successfully without errors or cancellation
**Then** the orchestrator sends `OrchestratorEvent::AllCompleted`
**And** displays the existing successful completion messages

#### Scenario: Parallel execution with genuine errors shows warning message

**Given** the orchestrator is running in parallel mode
**When** a non-cancellation execution error occurs
**And** all eligible queued work has been attempted
**Then** the orchestrator sends `OrchestratorEvent::AllCompleted`
**And** displays `Processing completed with errors`
**And** does not display a successful completion message

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

Parallel and serial execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace. The acceptance loop SHALL parse stdout to determine pass/fail/continue/stalled-hold outcomes, and MUST NOT use exit code as the acceptance verdict when a canonical verdict is present.

A completed acceptance command that emits no canonical verdict MUST be classified as an explicit missing-verdict protocol failure, not as an intentional `CONTINUE`. The runtime MUST record bounded output evidence and emit an actionable operator-visible diagnostic. Missing-verdict failures MUST NOT consume or enter the configured retry path reserved for explicit canonical `CONTINUE` verdicts.

During an active run, serial and parallel execution MUST instead apply the same dedicated missing-verdict protocol-retry policy. While the dedicated budget remains, runtime MUST invoke the normal configured acceptance command again with bounded Conflux-managed prior acceptance output, attempt evidence, current workspace context, and a trusted corrective instruction requiring exactly one canonical verdict. This continuation MUST NOT depend on harness session resume, harness-specific CLI flags, provider events, or external managed-job identifiers.

The dedicated policy MUST allow no more than two retries after the initial missing-verdict attempt. Its counter MUST be independent from explicit-`CONTINUE` accounting and MUST reset after any canonical verdict. Exhaustion MUST produce a terminal missing-verdict protocol failure with bounded evidence and attempt-count diagnostics. Acceptance command launch or execution failure MUST retain command-failure routing and MUST NOT be reclassified as a missing-verdict retry.

Acceptance execution MUST NOT create a workspace-root `ACCEPTANCE_REPORT.json` artifact or another generated retry checkpoint for PASS, command failure, FAIL, CONTINUE, stalled-hold, or missing-verdict outcomes. Acceptance outcomes MAY be recorded in active-run memory, events, or non-authoritative observability logs, but archive and resume routing MUST remain derivable from workspace file/git state. After process restart, an applied but unarchived workspace MUST run acceptance again and MUST NOT infer acceptance from prior narrative output.

When acceptance returns a stalled-hold compatibility verdict for infrastructure, external dependency, missing non-mockable credential, or pending-verification evidence, parallel execution SHALL record a non-terminal stalled hold and SHALL NOT invoke terminal rejection flow solely because of that verdict.

#### Scenario: status-only exit continues through a dedicated protocol retry

- **GIVEN** an acceptance command reports that it is waiting for owned verification
- **AND** the command exits without emitting a canonical verdict
- **AND** the active run has missing-verdict retry budget remaining
- **WHEN** acceptance execution classifies the completed command
- **THEN** the result remains an explicit missing-verdict protocol failure and is not classified as `CONTINUE`
- **AND** runtime records bounded evidence and invokes the normal acceptance command again
- **AND** the new prompt includes bounded prior attempt context plus a corrective canonical-verdict instruction
- **AND** the configured explicit-`CONTINUE` counter is unchanged
- **AND** queue reconciliation does not classify the active retry as `terminal_error_retry_required`

#### Scenario: continuation is harness neutral

- **GIVEN** any configured acceptance command can receive the normal Conflux prompt
- **WHEN** runtime retries after a missing verdict
- **THEN** continuity is provided only through Conflux-managed prompt context and workspace evidence
- **AND** runtime does not require a harness session ID, resume flag, provider event, or external job ID

#### Scenario: missing-verdict retry budget is exhausted

- **GIVEN** an initial acceptance attempt and two consecutive protocol retries all emit no canonical verdict
- **WHEN** runtime classifies the third consecutive missing verdict
- **THEN** it emits the terminal missing-verdict protocol failure
- **AND** the diagnostic identifies the exhausted attempts and includes bounded evidence
- **AND** no fourth protocol retry starts

#### Scenario: canonical outcome resets protocol retry state

- **GIVEN** acceptance previously entered a missing-verdict protocol retry
- **WHEN** a later invocation emits canonical PASS, FAIL, CONTINUE, or stalled-hold output
- **THEN** that canonical outcome retains its existing routing semantics
- **AND** the consecutive missing-verdict retry count resets
- **AND** explicit `CONTINUE` retains its configured continuation policy

#### Scenario: process restart uses workspace evidence

- **GIVEN** an acceptance protocol retry was active before process termination
- **AND** the workspace remains applied but unarchived
- **WHEN** Conflux restarts without out-of-worktree runtime state
- **THEN** it runs acceptance again from workspace file/git state
- **AND** it does not infer PASS from prior missing-verdict output
- **AND** it does not require a generated acceptance retry checkpoint

#### Scenario: missing verdict does not create report artifact

- **GIVEN** an acceptance command exits without a canonical verdict
- **WHEN** runtime records or retries the missing-verdict failure
- **THEN** the workspace root does not contain `ACCEPTANCE_REPORT.json` or another generated acceptance retry checkpoint
- **AND** evidence is carried through existing prompt, history, event, and observability paths

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

実行中の change が存在せず、queued の change も空の場合、オーケストレーションは完了状態にならなければならない（MUST）。ただし reducer-visible queued intent が存在する場合、その intent が terminal / active / missing などの理由で analysis 対象外であることが確認されるまで完了状態として扱ってはならない（MUST NOT）。

queued に含まれない change（例: merged 済み change、実行済み change、削除済み change）は analysis 対象から除外されなければならない（MUST）。

Archived-dirty repair candidate は workspace-derived repair trigger として扱われなければならない（MUST）。scheduler は同じ unchanged archived-dirty repair candidate の再発見を通常の user/reducer queue addition と同じ debounce 更新として扱ってはならない（MUST NOT）。

Reducer-terminal final states such as `merged`, `archived`, and `rejected` MUST be dispatch stop gates for ordinary apply/acceptance/archive work. A stale dynamic queue entry, stale scheduler-local candidate, or reducer reconciliation pass MUST NOT add a final terminal change to scheduler-local queued work or analysis candidates.

Recoverable terminal `error` remains distinct: it MUST NOT be dispatched through ordinary apply/acceptance/archive work unless explicit retry intent clears the terminal error according to reducer rules.

<!-- Expected canonical result after archive: `parallel-execution` will require scheduler queue ingestion, reconciliation, and dispatch selection to treat reducer-terminal final states as ordinary dispatch stop gates while preserving explicit retry semantics for terminal errors. -->

#### Scenario: terminal merged dynamic queue entry is ignored

- **GIVEN** change `alpha` is reducer-terminal `merged`
- **AND** a stale dynamic queue entry for `alpha` is popped
- **WHEN** scheduler dynamic queue ingestion evaluates `alpha`
- **THEN** `alpha` is not added to scheduler-local `queued`
- **AND** `alpha` is not included in dependency analysis candidates
- **AND** apply, acceptance, and archive are not started for `alpha`

#### Scenario: terminal merged dispatch preflight stops archive path

- **GIVEN** change `alpha` is reducer-terminal `merged`
- **AND** stale scheduler-local state attempts to dispatch `alpha`
- **WHEN** `dispatch_change_to_workspace` evaluates preflight guards
- **THEN** dispatch is skipped before workspace acquisition or reuse
- **AND** `execute_archive_in_workspace` is not called for `alpha`
- **AND** no `ArchiveStarted` event is emitted for `alpha`

#### Scenario: terminal error remains explicit retry only

- **GIVEN** change `beta` is reducer-terminal `error`
- **WHEN** ordinary scheduler dispatch evaluates `beta` without explicit retry intent
- **THEN** `beta` is skipped as retry-required
- **AND** apply, acceptance, and archive are not started for `beta`
- **WHEN** explicit retry intent clears the terminal error
- **THEN** `beta` can become eligible for ordinary queued dispatch again

### Requirement: Acceptance failure returns to apply loop

When acceptance returns FAIL, execution MUST permit at least one apply retry for repository-fixable findings regardless of whether the workspace started fresh or resumed. Before a later apply retry, runtime MUST compare the current normalized finding identity set and repository-visible semantic progress with the previous failed attempt. If the same findings recur after the permitted apply and no semantic progress exists, execution MUST enter a resumable stalled hold before invoking apply again.

Semantic progress MUST include substantive committed and uncommitted repository changes and MUST exclude runtime-managed acceptance follow-up content, blocker markers, attempt counters, logs, and observability-only state. Finding order, duplicates, and presentation-only whitespace MUST NOT create distinct identity sets. Previous finding identities, semantic baseline, and cycle count MUST be loaded from and updated through the workspace-local retry checkpoint so process restart cannot reset the retry decision.

#### Scenario: resumed workspace gets one repair attempt

- **GIVEN** a resumed Applied workspace runs acceptance
- **WHEN** acceptance returns repository-fixable FAIL findings for the first time
- **THEN** the next cycle runs apply before acceptance
- **AND** the change is not stalled solely because it resumed into acceptance

#### Scenario: repeated findings without progress stall before apply

- **GIVEN** acceptance returned FAIL and apply ran once
- **AND** the next acceptance returns the same normalized finding identity set
- **AND** the workspace has no semantic progress since the previous failed attempt
- **WHEN** runtime chooses the next action
- **THEN** it records `repeated_acceptance_findings` as a resumable stalled hold
- **AND** it does not invoke apply again or emit terminal Error solely for repetition

#### Scenario: real progress permits another bounded retry

- **GIVEN** acceptance returns the same finding identity after apply
- **AND** source, test, configuration, spec, or substantive task content changed
- **WHEN** runtime evaluates progress
- **THEN** the change remains eligible for another bounded apply retry
- **AND** runtime-owned bookkeeping alone would not produce this result

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

Parallel and serial execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace. The acceptance loop SHALL parse stdout to determine pass/fail/continue/stalled-hold outcomes, and MUST NOT use exit code as the acceptance verdict when a canonical verdict is present.

A completed acceptance command that emits no canonical verdict MUST be classified as an explicit missing-verdict protocol failure, not as an intentional `CONTINUE`. The runtime MUST record bounded output evidence and emit an actionable operator-visible diagnostic. Missing-verdict failures MUST NOT consume or enter the configured retry path reserved for explicit canonical `CONTINUE` verdicts.

During an active run, serial and parallel execution MUST instead apply the same dedicated missing-verdict protocol-retry policy. While the dedicated budget remains, runtime MUST invoke the normal configured acceptance command again with bounded Conflux-managed prior acceptance output, attempt evidence, current workspace context, and a trusted corrective instruction requiring exactly one canonical verdict. This continuation MUST NOT depend on harness session resume, harness-specific CLI flags, provider events, or external managed-job identifiers.

The dedicated policy MUST allow no more than two retries after the initial missing-verdict attempt. Its counter MUST be independent from explicit-`CONTINUE` accounting and MUST reset after any canonical verdict. Exhaustion MUST produce a terminal missing-verdict protocol failure with bounded evidence and attempt-count diagnostics. Acceptance command launch or execution failure MUST retain command-failure routing and MUST NOT be reclassified as a missing-verdict retry.

Acceptance execution MUST NOT create a workspace-root `ACCEPTANCE_REPORT.json` artifact or another generated retry checkpoint for PASS, command failure, FAIL, CONTINUE, stalled-hold, or missing-verdict outcomes. Acceptance outcomes MAY be recorded in active-run memory, events, or non-authoritative observability logs, but archive and resume routing MUST remain derivable from workspace file/git state. After process restart, an applied but unarchived workspace MUST run acceptance again and MUST NOT infer acceptance from prior narrative output.

When acceptance returns a stalled-hold compatibility verdict for infrastructure, external dependency, missing non-mockable credential, or pending-verification evidence, parallel execution SHALL record a non-terminal stalled hold and SHALL NOT invoke terminal rejection flow solely because of that verdict.

#### Scenario: status-only exit continues through a dedicated protocol retry

- **GIVEN** an acceptance command reports that it is waiting for owned verification
- **AND** the command exits without emitting a canonical verdict
- **AND** the active run has missing-verdict retry budget remaining
- **WHEN** acceptance execution classifies the completed command
- **THEN** the result remains an explicit missing-verdict protocol failure and is not classified as `CONTINUE`
- **AND** runtime records bounded evidence and invokes the normal acceptance command again
- **AND** the new prompt includes bounded prior attempt context plus a corrective canonical-verdict instruction
- **AND** the configured explicit-`CONTINUE` counter is unchanged
- **AND** queue reconciliation does not classify the active retry as `terminal_error_retry_required`

#### Scenario: continuation is harness neutral

- **GIVEN** any configured acceptance command can receive the normal Conflux prompt
- **WHEN** runtime retries after a missing verdict
- **THEN** continuity is provided only through Conflux-managed prompt context and workspace evidence
- **AND** runtime does not require a harness session ID, resume flag, provider event, or external job ID

#### Scenario: missing-verdict retry budget is exhausted

- **GIVEN** an initial acceptance attempt and two consecutive protocol retries all emit no canonical verdict
- **WHEN** runtime classifies the third consecutive missing verdict
- **THEN** it emits the terminal missing-verdict protocol failure
- **AND** the diagnostic identifies the exhausted attempts and includes bounded evidence
- **AND** no fourth protocol retry starts

#### Scenario: canonical outcome resets protocol retry state

- **GIVEN** acceptance previously entered a missing-verdict protocol retry
- **WHEN** a later invocation emits canonical PASS, FAIL, CONTINUE, or stalled-hold output
- **THEN** that canonical outcome retains its existing routing semantics
- **AND** the consecutive missing-verdict retry count resets
- **AND** explicit `CONTINUE` retains its configured continuation policy

#### Scenario: process restart uses workspace evidence

- **GIVEN** an acceptance protocol retry was active before process termination
- **AND** the workspace remains applied but unarchived
- **WHEN** Conflux restarts without out-of-worktree runtime state
- **THEN** it runs acceptance again from workspace file/git state
- **AND** it does not infer PASS from prior missing-verdict output
- **AND** it does not require a generated acceptance retry checkpoint

#### Scenario: missing verdict does not create report artifact

- **GIVEN** an acceptance command exits without a canonical verdict
- **WHEN** runtime records or retries the missing-verdict failure
- **THEN** the workspace root does not contain `ACCEPTANCE_REPORT.json` or another generated acceptance retry checkpoint
- **AND** evidence is carried through existing prompt, history, event, and observability paths

### Requirement: Parallel execution acceptance loop

Parallel and serial execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace. The acceptance loop SHALL parse stdout to determine pass/fail/continue/stalled-hold outcomes, and MUST NOT use exit code as the acceptance verdict when a canonical verdict is present.

A completed acceptance command that emits no canonical verdict MUST be classified as an explicit missing-verdict protocol failure, not as an intentional `CONTINUE`. The runtime MUST record bounded output evidence and emit an actionable operator-visible diagnostic. Missing-verdict failures MUST NOT consume or enter the configured retry path reserved for explicit canonical `CONTINUE` verdicts.

During an active run, serial and parallel execution MUST instead apply the same dedicated missing-verdict protocol-retry policy. While the dedicated budget remains, runtime MUST invoke the normal configured acceptance command again with bounded Conflux-managed prior acceptance output, attempt evidence, current workspace context, and a trusted corrective instruction requiring exactly one canonical verdict. This continuation MUST NOT depend on harness session resume, harness-specific CLI flags, provider events, or external managed-job identifiers.

The dedicated policy MUST allow no more than two retries after the initial missing-verdict attempt. Its counter MUST be independent from explicit-`CONTINUE` accounting and MUST reset after any canonical verdict. Exhaustion MUST produce a terminal missing-verdict protocol failure with bounded evidence and attempt-count diagnostics. Acceptance command launch or execution failure MUST retain command-failure routing and MUST NOT be reclassified as a missing-verdict retry.

Acceptance execution MUST NOT create a workspace-root `ACCEPTANCE_REPORT.json` artifact or another generated retry checkpoint for PASS, command failure, FAIL, CONTINUE, stalled-hold, or missing-verdict outcomes. Acceptance outcomes MAY be recorded in active-run memory, events, or non-authoritative observability logs, but archive and resume routing MUST remain derivable from workspace file/git state. After process restart, an applied but unarchived workspace MUST run acceptance again and MUST NOT infer acceptance from prior narrative output.

When acceptance returns a stalled-hold compatibility verdict for infrastructure, external dependency, missing non-mockable credential, or pending-verification evidence, parallel execution SHALL record a non-terminal stalled hold and SHALL NOT invoke terminal rejection flow solely because of that verdict.

#### Scenario: status-only exit continues through a dedicated protocol retry

- **GIVEN** an acceptance command reports that it is waiting for owned verification
- **AND** the command exits without emitting a canonical verdict
- **AND** the active run has missing-verdict retry budget remaining
- **WHEN** acceptance execution classifies the completed command
- **THEN** the result remains an explicit missing-verdict protocol failure and is not classified as `CONTINUE`
- **AND** runtime records bounded evidence and invokes the normal acceptance command again
- **AND** the new prompt includes bounded prior attempt context plus a corrective canonical-verdict instruction
- **AND** the configured explicit-`CONTINUE` counter is unchanged
- **AND** queue reconciliation does not classify the active retry as `terminal_error_retry_required`

#### Scenario: continuation is harness neutral

- **GIVEN** any configured acceptance command can receive the normal Conflux prompt
- **WHEN** runtime retries after a missing verdict
- **THEN** continuity is provided only through Conflux-managed prompt context and workspace evidence
- **AND** runtime does not require a harness session ID, resume flag, provider event, or external job ID

#### Scenario: missing-verdict retry budget is exhausted

- **GIVEN** an initial acceptance attempt and two consecutive protocol retries all emit no canonical verdict
- **WHEN** runtime classifies the third consecutive missing verdict
- **THEN** it emits the terminal missing-verdict protocol failure
- **AND** the diagnostic identifies the exhausted attempts and includes bounded evidence
- **AND** no fourth protocol retry starts

#### Scenario: canonical outcome resets protocol retry state

- **GIVEN** acceptance previously entered a missing-verdict protocol retry
- **WHEN** a later invocation emits canonical PASS, FAIL, CONTINUE, or stalled-hold output
- **THEN** that canonical outcome retains its existing routing semantics
- **AND** the consecutive missing-verdict retry count resets
- **AND** explicit `CONTINUE` retains its configured continuation policy

#### Scenario: process restart uses workspace evidence

- **GIVEN** an acceptance protocol retry was active before process termination
- **AND** the workspace remains applied but unarchived
- **WHEN** Conflux restarts without out-of-worktree runtime state
- **THEN** it runs acceptance again from workspace file/git state
- **AND** it does not infer PASS from prior missing-verdict output
- **AND** it does not require a generated acceptance retry checkpoint

#### Scenario: missing verdict does not create report artifact

- **GIVEN** an acceptance command exits without a canonical verdict
- **WHEN** runtime records or retries the missing-verdict failure
- **THEN** the workspace root does not contain `ACCEPTANCE_REPORT.json` or another generated acceptance retry checkpoint
- **AND** evidence is carried through existing prompt, history, event, and observability paths

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

When a reducer-owned `ResolveWait` retry is evaluated while the base repository is dirty and no other non-terminal change is actively `Resolving` or `Rejecting`, the scheduler SHALL classify the retry as manual-intervention merge wait and feed a concrete manual deferral back into the reducer. The change MUST transition from `resolve pending` to `merge wait`, and it MUST be removed from reducer-owned resolve-wait queues.

When a reducer-owned `ResolveWait` retry is evaluated after the base repository becomes clean and the base-mutating lane is free, the scheduler SHALL retry or promote the pending merge without requiring another `M` keypress. The change MUST NOT remain indefinitely in `resolve pending` solely because the previous evaluation observed a dirty base repository.

When a scheduler is started with zero normal queued changes solely to consume reducer-owned manual merge retry intent, it MUST treat existing `ResolveWait` / `RejectWait` membership as scheduler work. It MUST synchronize that membership from the caller-owned shared reducer state before idle or completion decisions, evaluate at least one eligible lane-wait retry, and MUST NOT complete as a zero-change success while shared lane-wait membership remains pending or active.

If retry evaluation observes stale, missing, or manually blocked retry prerequisites, the scheduler MUST feed visible reducer evidence that clears scheduler-owned pending membership and transitions the change to `merge wait` or an explicit error/stalled state with a reason. It MUST NOT leave a change indefinitely visible as `resolve pending` when no scheduler-consumable retry work remains.

Internal helper names or comments used by this retry-clearing path SHOULD describe stale, missing, already-merged, and success outcomes neutrally. They MUST NOT make stale or missing workspace cleanup look like successful merge completion.

Canonical rule: `M` is **intent-only** (`ResolveWait` request in shared reducer state), scheduler loop is the **sole execution owner** for merge/resolve retry start, and reducer completion events (`ResolveCompleted`/`ResolveFailed`/`MergeDeferred`/`MergeCompleted`) are the **sole authority** for clearing or transitioning wait state.

<!-- Expected canonical result after archive: `parallel-execution` will clarify that retry intent clearing helpers should describe outcome semantics rather than success-only semantics. -->

#### Scenario: M key registers retry intent instead of direct execution

- **GIVEN** change `alpha` is in `MergeWait`
- **WHEN** the user presses `M`
- **THEN** the system records scheduler-visible retry intent for `alpha`
- **AND** the TUI command path does not directly execute `resolve_deferred_merge(...)`

#### Scenario: stale retry evidence clears resolve pending visibly

- **GIVEN** change `alpha` is in reducer-owned `ResolveWait`
- **AND** the archived workspace path required for retry is missing or stale
- **WHEN** the scheduler evaluates pending base-mutating lane waiters for `alpha`
- **THEN** scheduler-owned `ResolveWait(alpha)` is cleared
- **AND** `alpha` becomes visible as `merge wait` or explicit error/stalled state with a reason
- **AND** `alpha` does not remain indefinitely in `resolve pending`

#### Scenario: retry-clearing helper wording is outcome-neutral

- **GIVEN** the scheduler clears `ResolveWait` for an already-merged, missing-workspace, stale-workspace, or successful-merge outcome
- **WHEN** a maintainer reads the helper name or comments in the retry-clearing path
- **THEN** the wording indicates a terminal/no-longer-retryable outcome
- **AND** it does not describe stale or missing workspace cleanup as success

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

Parallel and serial execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace. The acceptance loop SHALL parse stdout to determine pass/fail/continue/stalled-hold outcomes, and MUST NOT use exit code as the acceptance verdict when a canonical verdict is present.

A completed acceptance command that emits no canonical verdict MUST be classified as an explicit missing-verdict protocol failure, not as an intentional `CONTINUE`. The runtime MUST record bounded output evidence and emit an actionable operator-visible diagnostic. Missing-verdict failures MUST NOT consume or enter the configured retry path reserved for explicit canonical `CONTINUE` verdicts.

During an active run, serial and parallel execution MUST instead apply the same dedicated missing-verdict protocol-retry policy. While the dedicated budget remains, runtime MUST invoke the normal configured acceptance command again with bounded Conflux-managed prior acceptance output, attempt evidence, current workspace context, and a trusted corrective instruction requiring exactly one canonical verdict. This continuation MUST NOT depend on harness session resume, harness-specific CLI flags, provider events, or external managed-job identifiers.

The dedicated policy MUST allow no more than two retries after the initial missing-verdict attempt. Its counter MUST be independent from explicit-`CONTINUE` accounting and MUST reset after any canonical verdict. Exhaustion MUST produce a terminal missing-verdict protocol failure with bounded evidence and attempt-count diagnostics. Acceptance command launch or execution failure MUST retain command-failure routing and MUST NOT be reclassified as a missing-verdict retry.

Acceptance execution MUST NOT create a workspace-root `ACCEPTANCE_REPORT.json` artifact or another generated retry checkpoint for PASS, command failure, FAIL, CONTINUE, stalled-hold, or missing-verdict outcomes. Acceptance outcomes MAY be recorded in active-run memory, events, or non-authoritative observability logs, but archive and resume routing MUST remain derivable from workspace file/git state. After process restart, an applied but unarchived workspace MUST run acceptance again and MUST NOT infer acceptance from prior narrative output.

When acceptance returns a stalled-hold compatibility verdict for infrastructure, external dependency, missing non-mockable credential, or pending-verification evidence, parallel execution SHALL record a non-terminal stalled hold and SHALL NOT invoke terminal rejection flow solely because of that verdict.

#### Scenario: status-only exit continues through a dedicated protocol retry

- **GIVEN** an acceptance command reports that it is waiting for owned verification
- **AND** the command exits without emitting a canonical verdict
- **AND** the active run has missing-verdict retry budget remaining
- **WHEN** acceptance execution classifies the completed command
- **THEN** the result remains an explicit missing-verdict protocol failure and is not classified as `CONTINUE`
- **AND** runtime records bounded evidence and invokes the normal acceptance command again
- **AND** the new prompt includes bounded prior attempt context plus a corrective canonical-verdict instruction
- **AND** the configured explicit-`CONTINUE` counter is unchanged
- **AND** queue reconciliation does not classify the active retry as `terminal_error_retry_required`

#### Scenario: continuation is harness neutral

- **GIVEN** any configured acceptance command can receive the normal Conflux prompt
- **WHEN** runtime retries after a missing verdict
- **THEN** continuity is provided only through Conflux-managed prompt context and workspace evidence
- **AND** runtime does not require a harness session ID, resume flag, provider event, or external job ID

#### Scenario: missing-verdict retry budget is exhausted

- **GIVEN** an initial acceptance attempt and two consecutive protocol retries all emit no canonical verdict
- **WHEN** runtime classifies the third consecutive missing verdict
- **THEN** it emits the terminal missing-verdict protocol failure
- **AND** the diagnostic identifies the exhausted attempts and includes bounded evidence
- **AND** no fourth protocol retry starts

#### Scenario: canonical outcome resets protocol retry state

- **GIVEN** acceptance previously entered a missing-verdict protocol retry
- **WHEN** a later invocation emits canonical PASS, FAIL, CONTINUE, or stalled-hold output
- **THEN** that canonical outcome retains its existing routing semantics
- **AND** the consecutive missing-verdict retry count resets
- **AND** explicit `CONTINUE` retains its configured continuation policy

#### Scenario: process restart uses workspace evidence

- **GIVEN** an acceptance protocol retry was active before process termination
- **AND** the workspace remains applied but unarchived
- **WHEN** Conflux restarts without out-of-worktree runtime state
- **THEN** it runs acceptance again from workspace file/git state
- **AND** it does not infer PASS from prior missing-verdict output
- **AND** it does not require a generated acceptance retry checkpoint

#### Scenario: missing verdict does not create report artifact

- **GIVEN** an acceptance command exits without a canonical verdict
- **WHEN** runtime records or retries the missing-verdict failure
- **THEN** the workspace root does not contain `ACCEPTANCE_REPORT.json` or another generated acceptance retry checkpoint
- **AND** evidence is carried through existing prompt, history, event, and observability paths

### Requirement: Applied resume uses workspace-local evidence only

Parallel execution MUST determine resume routing from workspace-local file state, Git state, and base-branch tree evidence only.

For implementation changes, if implementation tasks are incomplete, resume routing MUST return to apply.

Otherwise, a complete implementation that is not repository-verifiably archived or base-integrated MUST run acceptance before archive. Conflux MUST NOT create or consult a generated acceptance checkpoint to infer prior PASS after restart.

An archived or base-integrated workspace MAY continue to post-archive resolve, merge, or terminal handling without rerunning acceptance.

Out-of-worktree durable state (for example under `~/.local/state/cflx/**`) MUST NOT be used as authoritative input for this decision.

#### Scenario: applied workspace resumes acceptance regardless of external durable state

- **GIVEN** a workspace is detected as `Applied`
- **AND** implementation tasks are complete
- **AND** external durable acceptance/archive state files exist or do not exist
- **WHEN** resume routing is evaluated
- **THEN** the next action is `Acceptance`
- **AND** the result is identical regardless of external state presence
- **AND** `.cflx/acceptance-state.json` is not created or consulted

#### Scenario: applied workspace with incomplete implementation tasks resumes apply

- **GIVEN** a workspace is detected as `Applied`
- **AND** implementation tasks are incomplete
- **WHEN** resume routing is evaluated
- **THEN** the next action is `Apply`
- **AND** acceptance/archive are not entered in that cycle

#### Scenario: interrupted incomplete archive reruns acceptance

- **GIVEN** archive work began but repository evidence does not prove a complete valid archive
- **AND** no resumable blocker marker prevents dispatch
- **WHEN** execution resumes after process restart
- **THEN** acceptance runs before archive finalization
- **AND** a prior PASS is not inferred from missing generated state

#### Scenario: repository-verifiably archived workspace continues post-archive handling

- **GIVEN** the active change directory is absent
- **AND** a valid archive entry exists
- **WHEN** resume routing is evaluated
- **THEN** the change continues to resolve, merge, or terminal handling as appropriate
- **AND** acceptance checkpoint state is not required

### Requirement: post-archive-merge-dispatch

If `on_merged` fails because the root repository is not safe for repo-mutating hook execution, such as root `.git/index.lock` contention, Conflux SHALL treat that as a hook failure that blocks merged transition when `continue_on_failure=false`.

A deferred merge caused by another active non-terminal change in `Resolving` or `Rejecting` SHALL advance into reducer-owned auto-resumable merge/resolve handling (`ResolveWait` or immediate resolving when promoted). Active `Rejecting` is included because rejection review can touch and dirty base state.

A deferred merge caused by active `Applying`, `Accepting`, `Archiving`, terminal `Rejected`, dirty base without an active base-mutating lane occupant, or other manual intervention requirement SHALL NOT be classified as automatic `ResolveWait` solely because that state exists. Dirty base and manual intervention deferrals SHALL remain in manual merge wait handling (`MergeWait`).

The implementation MUST NOT infer auto-resumable versus manual-wait behavior by parsing a human-readable deferred reason string.

A change already in reducer-owned `ResolveWait` MUST follow the same classification rules when its retry is evaluated: active `Resolving` or `Rejecting` by another change remains auto-resumable, while dirty base without an active base-mutating lane occupant demotes to manual `MergeWait`.

<!-- Expected canonical result after archive: `parallel-execution` will explicitly require retry-time `ResolveWait` classification to match post-archive merge deferral classification. -->

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

#### Scenario: dirty-base ResolveWait retry demotes to merge wait

**Given**: change `alpha` is already in reducer-owned `ResolveWait`
**And**: no other change is actively `Resolving` or `Rejecting`
**And**: merge retry is deferred because the base branch is dirty
**When**: the deferred retry result is processed
**Then**: `alpha` transitions to manual merge wait handling (`MergeWait`)
**And**: `alpha` is no longer treated as auto-resumable retry work

#### Scenario: root index lock contention blocks merged transition

**Given**: change `alpha` is repository-visible merged
**And**: `hooks.on_merged` runs a repo-mutating command such as `make bump-patch`
**And**: root `.git/index.lock` contention causes the hook to exit non-zero
**When**: the scheduler handles hook completion
**Then**: `alpha` does not transition to terminal `Merged`
**And**: `MergeCompleted` is not emitted for `alpha`
**And**: the operator-visible failure context includes the hook failure details

### Requirement: Parallel execution acceptance loop

Parallel and serial execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace. The acceptance loop SHALL parse stdout to determine pass/fail/continue/stalled-hold outcomes, and MUST NOT use exit code as the acceptance verdict when a canonical verdict is present.

A completed acceptance command that emits no canonical verdict MUST be classified as an explicit missing-verdict protocol failure, not as an intentional `CONTINUE`. The runtime MUST record bounded output evidence and emit an actionable operator-visible diagnostic. Missing-verdict failures MUST NOT consume or enter the configured retry path reserved for explicit canonical `CONTINUE` verdicts.

During an active run, serial and parallel execution MUST instead apply the same dedicated missing-verdict protocol-retry policy. While the dedicated budget remains, runtime MUST invoke the normal configured acceptance command again with bounded Conflux-managed prior acceptance output, attempt evidence, current workspace context, and a trusted corrective instruction requiring exactly one canonical verdict. This continuation MUST NOT depend on harness session resume, harness-specific CLI flags, provider events, or external managed-job identifiers.

The dedicated policy MUST allow no more than two retries after the initial missing-verdict attempt. Its counter MUST be independent from explicit-`CONTINUE` accounting and MUST reset after any canonical verdict. Exhaustion MUST produce a terminal missing-verdict protocol failure with bounded evidence and attempt-count diagnostics. Acceptance command launch or execution failure MUST retain command-failure routing and MUST NOT be reclassified as a missing-verdict retry.

Acceptance execution MUST NOT create a workspace-root `ACCEPTANCE_REPORT.json` artifact or another generated retry checkpoint for PASS, command failure, FAIL, CONTINUE, stalled-hold, or missing-verdict outcomes. Acceptance outcomes MAY be recorded in active-run memory, events, or non-authoritative observability logs, but archive and resume routing MUST remain derivable from workspace file/git state. After process restart, an applied but unarchived workspace MUST run acceptance again and MUST NOT infer acceptance from prior narrative output.

When acceptance returns a stalled-hold compatibility verdict for infrastructure, external dependency, missing non-mockable credential, or pending-verification evidence, parallel execution SHALL record a non-terminal stalled hold and SHALL NOT invoke terminal rejection flow solely because of that verdict.

#### Scenario: status-only exit continues through a dedicated protocol retry

- **GIVEN** an acceptance command reports that it is waiting for owned verification
- **AND** the command exits without emitting a canonical verdict
- **AND** the active run has missing-verdict retry budget remaining
- **WHEN** acceptance execution classifies the completed command
- **THEN** the result remains an explicit missing-verdict protocol failure and is not classified as `CONTINUE`
- **AND** runtime records bounded evidence and invokes the normal acceptance command again
- **AND** the new prompt includes bounded prior attempt context plus a corrective canonical-verdict instruction
- **AND** the configured explicit-`CONTINUE` counter is unchanged
- **AND** queue reconciliation does not classify the active retry as `terminal_error_retry_required`

#### Scenario: continuation is harness neutral

- **GIVEN** any configured acceptance command can receive the normal Conflux prompt
- **WHEN** runtime retries after a missing verdict
- **THEN** continuity is provided only through Conflux-managed prompt context and workspace evidence
- **AND** runtime does not require a harness session ID, resume flag, provider event, or external job ID

#### Scenario: missing-verdict retry budget is exhausted

- **GIVEN** an initial acceptance attempt and two consecutive protocol retries all emit no canonical verdict
- **WHEN** runtime classifies the third consecutive missing verdict
- **THEN** it emits the terminal missing-verdict protocol failure
- **AND** the diagnostic identifies the exhausted attempts and includes bounded evidence
- **AND** no fourth protocol retry starts

#### Scenario: canonical outcome resets protocol retry state

- **GIVEN** acceptance previously entered a missing-verdict protocol retry
- **WHEN** a later invocation emits canonical PASS, FAIL, CONTINUE, or stalled-hold output
- **THEN** that canonical outcome retains its existing routing semantics
- **AND** the consecutive missing-verdict retry count resets
- **AND** explicit `CONTINUE` retains its configured continuation policy

#### Scenario: process restart uses workspace evidence

- **GIVEN** an acceptance protocol retry was active before process termination
- **AND** the workspace remains applied but unarchived
- **WHEN** Conflux restarts without out-of-worktree runtime state
- **THEN** it runs acceptance again from workspace file/git state
- **AND** it does not infer PASS from prior missing-verdict output
- **AND** it does not require a generated acceptance retry checkpoint

#### Scenario: missing verdict does not create report artifact

- **GIVEN** an acceptance command exits without a canonical verdict
- **WHEN** runtime records or retries the missing-verdict failure
- **THEN** the workspace root does not contain `ACCEPTANCE_REPORT.json` or another generated acceptance retry checkpoint
- **AND** evidence is carried through existing prompt, history, event, and observability paths

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

The canonical parser MAY continue to accept `gated` and legacy `blocked` verdict input for compatibility, but runtime MUST distinguish bare compatibility input from a validated structured external blocker. Bare input without explicit supported category, concrete non-empty evidence, next action, and resumability MUST be treated as an Acceptance protocol error and MUST NOT create `stalled`, dependency `blocked`, or an inferred blocker category.

A validated external blocker SHALL enter the non-terminal user-facing `stalled` lifecycle. Newly authored lifecycle/status surfaces MUST NOT expose `gated` as a status. Dependency wait remains the only `blocked` display meaning.

#### Scenario: bare gated input receives bounded protocol retry

- **GIVEN** Acceptance emits `{"acceptance":"gated"}` or `ACCEPTANCE: GATED` without a structured blocker payload
- **WHEN** runtime parses and routes the result
- **THEN** it classifies the result as an Acceptance protocol error
- **AND** it retries Acceptance only within the shared fixed protocol budget
- **AND** it emits no stalled lifecycle transition or blocker category
- **AND** it creates no change artifact or durable stalled record

#### Scenario: legacy bare blocked input is compatibility-only

- **GIVEN** an older integration emits a bare `blocked` Acceptance verdict
- **WHEN** a compatibility-aware runtime parses it
- **THEN** it follows the same bounded protocol-error path as bare `gated`
- **AND** it is not displayed as dependency `blocked` or execution `stalled`

#### Scenario: validated blocker displays as stalled

- **GIVEN** Acceptance emits a blocker with an explicit supported category, concrete evidence, next action, and resumability
- **AND** runtime verifies that repository-only Apply work cannot resolve the prerequisite
- **WHEN** runtime exposes lifecycle state
- **THEN** the displayed status is `stalled`
- **AND** the explicit category is preserved without prose-based inference
- **AND** new prompts and tests do not require `gated` as a lifecycle/display term

### Requirement: archived dependency references have explicit scheduler and validation semantics

Archived, active, queued, in-flight, missing, and rejected dependency targets MUST remain explicitly classified during analysis and scheduler dispatch. Fallback analysis after an LLM dependency-analysis failure MUST remain metadata-dependency-only rather than dependency-free. When fallback succeeds, the failed LLM attempt is a degraded analysis diagnostic, not a terminal workflow error.

#### Scenario: fallback analysis preserves metadata dependency

- **GIVEN** `route` has proposal metadata dependency `policy`
- **AND** LLM analysis fails or is disabled
- **WHEN** fallback analysis creates an order result
- **THEN** the fallback result includes `route -> policy`
- **AND** the fallback is metadata-dependency-only rather than dependency-free
- **AND** a successful fallback path is not reported as a terminal error-level workflow failure

#### Scenario: missing dependency fails closed

- **GIVEN** active change `route` references dependency `ghost`
- **AND** `ghost` exists neither in the queued set, nor the in-flight set, nor the archive tree
- **WHEN** analyzer validation or scheduler dispatch checks evaluate `route`
- **THEN** `ghost` is classified as missing
- **AND** `route` is not dispatched
- **AND** the diagnostic distinguishes missing dependency from archived dependency
- **AND** the unsafe dependency blocker remains visible as actionable operator evidence

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

When acceptance returns a non-pass verdict with findings and retry policy routes the change back to apply, the runtime SHALL preserve that acceptance verdict as the primary outcome even if follow-up persistence into `tasks.md` degrades.

For an apply-retry outcome, the runtime SHALL attempt to persist acceptance follow-up findings to the canonical tasks location for the workspace. It MUST prefer the active change tasks path and MUST fall back to the matching archived tasks path when the active path does not exist. A FAIL routed directly to a resumable stalled hold MAY preserve current findings in its workspace checkpoint and stalled marker without updating `tasks.md`.

Runtime MUST be the sole writer of numbered `## Acceptance #<n> Failure Follow-up` sections. For apply-retry outcomes it MUST retain only the latest runtime-owned section, normalize multiline findings into one checkbox task per finding, rehydrate deleted or altered runtime findings during apply, and remove the runtime-owned section after acceptance PASS. Serial and parallel execution MUST apply the same persistence and cleanup behavior.

Runtime MUST ignore matching headings inside fenced code examples. When a detected runtime-owned section has an unambiguous boundary but contains content outside the supported runtime record forms, runtime MUST preserve the unknown content byte-for-byte outside the runtime-owned section under `## Recovered Acceptance Notes`, enclose it in a dynamically sized fenced literal, emit supplemental recovery diagnostics, and continue replacement or cleanup. The recovered representation MUST identify the payload as untrusted content that is neither instructions nor task state, MUST deduplicate identical payload bytes across retries and restarts, and MUST remain after acceptance PASS cleanup. Preservation and runtime-section replacement or removal MUST occur in one atomic tasks-file update.

Runtime MUST refuse the destructive update and leave `tasks.md` unchanged when the runtime-owned boundary cannot be determined safely, including an unclosed fence or ambiguous layout, or when unknown content cannot be preserved before replacement. Failure to persist or recover follow-up findings MUST NOT by itself convert an acceptance `FAIL` into a terminal execution `Error` unless the primary acceptance outcome itself is indeterminate.

Task progress and OpenSpec task validation MUST ignore checkbox-like content inside valid backtick or tilde fenced blocks so recovered content cannot alter completion or archive decisions.

If persistence degrades, the runtime SHALL record the explored path(s) and expose the persistence issue as supplemental warning/error context rather than replacing the acceptance diagnosis.

#### Scenario: unknown follow-up prose is preserved and normalized

- **GIVEN** a runtime-owned acceptance follow-up has an unambiguous boundary
- **AND** it contains supported runtime findings plus unknown multiline evidence or presentation text
- **WHEN** runtime replaces the follow-up for a retry
- **THEN** runtime preserves the unknown bytes in one fenced recovered-notes block
- **AND** runtime writes the canonical current follow-up from normalized findings
- **AND** execution continues with a supplemental warning rather than a terminal configuration error

#### Scenario: repeated recovery is idempotent

- **GIVEN** unknown follow-up content has already been moved to recovered notes
- **WHEN** apply hydration, retry, or process restart normalizes the same findings again
- **THEN** the same recovered payload is not appended a second time
- **AND** workspace-derived follow-up state remains deterministic

#### Scenario: pass cleanup retains recovered notes

- **GIVEN** a current runtime-owned follow-up and previously recovered notes exist
- **WHEN** acceptance returns PASS and runtime performs cleanup
- **THEN** the runtime-owned follow-up is removed
- **AND** recovered notes remain as non-task repository evidence

#### Scenario: recovered checkbox text is inert

- **GIVEN** recovered content contains headings and `- [ ]` or `- [x]` text inside a valid fenced literal
- **WHEN** Conflux calculates task progress or performs strict and archive-gate task validation
- **THEN** fenced checkbox text does not change task totals or completion totals
- **AND** fenced content does not create implementation-task validation findings

#### Scenario: ambiguous boundary remains a hard error

- **GIVEN** a possible runtime-owned follow-up contains an unclosed fence or another layout that prevents safe boundary determination
- **WHEN** runtime attempts replacement or PASS cleanup
- **THEN** runtime leaves the original tasks file byte-for-byte unchanged
- **AND** reports an actionable hard error identifying the structural ambiguity

#### Scenario: failed preservation does not destroy content

- **GIVEN** an unambiguous runtime-owned follow-up contains unknown content
- **AND** runtime cannot complete the atomic recovered-notes update
- **WHEN** replacement or cleanup is attempted
- **THEN** the original tasks file remains unchanged
- **AND** an acceptance FAIL remains the primary diagnosis while persistence degradation is supplemental

<!-- Expected canonical result after archive: acceptance follow-up updates preserve recoverable unknown content in inert workspace-local notes, remain idempotent and atomic, keep fenced text out of task accounting, and reserve hard errors for unsafe boundaries or failed preservation. -->

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

### Requirement: Apply completion grace requires stable repository completion

When runtime observes an apply completion condition while the apply child is still running, it MAY start a bounded grace period before terminating the child. Runtime MUST re-evaluate the same repository completion condition when the grace period expires and MUST terminate the child only if that condition remains present. If completion disappears or changes during the grace period, runtime MUST cancel or restart the deadline for the current condition and continue apply.

#### Scenario: transient task completion does not terminate apply

- **GIVEN** `tasks.md` becomes complete while the apply child remains running
- **AND** runtime starts its completion grace period
- **AND** `tasks.md` becomes incomplete before the grace period expires
- **WHEN** runtime rechecks repository state at the deadline
- **THEN** it does not terminate the child based on the stale completion observation
- **AND** apply continues until a completion condition remains stable or the child exits

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

Diagnosis output is supplemental evidence only. A successful diagnosis command MAY repair repository state; when repository-verifiable task progress is complete after the command, runtime MUST clear the stall and continue the successful apply path. Otherwise diagnosis MUST NOT replace the primary empty-WIP stall reason.

#### Scenario: diagnosis runs once before final stall

- **GIVEN** a change has exhausted its allowed escalation retries
- **AND** the consecutive empty WIP count reaches the configured final stall threshold
- **AND** `apply_stall_diagnose_command` is configured
- **WHEN** the runtime finalizes the empty-WIP stall
- **THEN** it executes `apply_stall_diagnose_command` exactly once
- **AND** it records diagnosis output as diagnostic evidence/logging
- **AND** if the command does not both succeed and leave repository-verifiable task progress complete, the final stall outcome still reports the empty-WIP stall as the primary reason

#### Scenario: successful diagnosis repair completes apply

- **GIVEN** the final empty-WIP threshold is reached
- **AND** the configured diagnosis command exits successfully
- **AND** repository-verifiable task progress is complete after diagnosis
- **WHEN** runtime rechecks the workspace
- **THEN** it clears the apply stall state
- **AND** apply continues to the normal completion handoff

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

### Requirement: Dynamic queue ingestion resolves changes from the configured repository root

スケジューラの dynamic queue ingestion における候補 change の検証は、プロセスの
カレントディレクトリではなく、orchestrator/executor に設定された repository root を
基準に OpenSpec change を解決しなければならない（MUST）。`repo_root` がプロセス cwd と
異なる場合でも、ingestion の判定結果は `repo_root` 配下の `openspec/changes/` の内容
のみに依存しなければならない（MUST）。

候補 id が `repo_root` 配下の active change として存在しない場合、ingestion は既存の
`candidate_not_found` reconciliation ログを発行し、scheduler-local queued へ追加して
はならない（MUST NOT）。

この要件の回帰カバレッジは、ホストリポジトリ自身の OpenSpec change 内容（active /
archive 状態）に依存しない self-contained な fixture で検証可能でなければならない
（MUST）。

#### Scenario: Candidate present only under the configured repo root is ingested

- **GIVEN** executor が temp ディレクトリ R を `repo_root` として構成され、プロセス cwd は R と異なる
- **AND** change `synthetic-change` が R の `openspec/changes/synthetic-change/` にのみ存在する
- **WHEN** dynamic queue に `synthetic-change` が push され ingestion が評価される
- **THEN** `synthetic-change` は scheduler-local queued へ取り込まれる
- **AND** "Dynamically added to parallel execution" のログイベントが発行される

#### Scenario: Candidate absent under the configured repo root is not queued

- **GIVEN** executor が temp ディレクトリ R を `repo_root` として構成されている
- **AND** 候補 id `missing-change` が R 配下の active change として存在しない（プロセス cwd 配下の存在有無に関わらず）
- **WHEN** dynamic queue から `missing-change` が pop され ingestion が評価される
- **THEN** `missing-change` は scheduler-local queued へ追加されない
- **AND** `candidate_not_found` の reconciliation ログが発行される

### Requirement: Reanalysis dispatch guards are factored by responsibility

The scheduler's reanalysis-and-dispatch path SHALL be decomposed into single-responsibility guard and action helpers instead of one monolithic function. The top-level reanalysis function SHALL read as an orchestration skeleton and delegate classification, reanalysis reason computation, debounce evaluation, executable filtering, analysis execution, post-analysis capacity handling, and dispatch execution to explicit helpers.

#### Scenario: Queue notification debounce behavior remains unchanged after extraction

- **GIVEN** `last_queue_change_at` is fresh
- **AND** scheduler iteration is greater than 1
- **AND** the reanalysis reason is `QueueNotification`
- **WHEN** the refactored reanalysis path evaluates whether to analyze
- **THEN** analysis starts immediately without waiting for the debounce window
- **AND** an `AnalysisStarted` event is emitted

#### Scenario: Zero-capacity behavior remains unchanged after extraction

- **GIVEN** queued dispatchable work exists
- **AND** `in_flight.len() == max_parallelism`
- **WHEN** the refactored reanalysis path runs dependency analysis
- **THEN** dependency analysis still runs
- **AND** ordinary apply dispatch is suppressed
- **AND** the capacity-zero diagnostic is emitted through the diagnostic deduplication store

#### Scenario: Blocked-only work skips analyzer after extraction

- **GIVEN** queued work is entirely merge-wait or terminal-error blocked
- **WHEN** the refactored reanalysis path classifies queued work
- **THEN** the dependency analyzer is not invoked
- **AND** a no-analysis diagnostic is emitted through the diagnostic deduplication store

### Requirement: Dependency classification logic is centralized

The scheduler SHALL maintain a single `DependencyContext` implementation that encapsulates the construction of queued, in-flight, active, archived, rejected, and terminal-error lookup sets, as well as the `classify_dependency_target` and `effective_dependency_base` decision logic. `classify_queued_work`, `select_changes_for_dispatch`, and any future callers SHALL delegate to this shared context rather than duplicating HashSet construction or classification loops.

#### Scenario: Archived dependency uses effective base after branch switch

- **GIVEN** a change is archived on the `integration` branch but not on `main`
- **AND** the executor `repo_root` is on the `integration` branch
- **WHEN** the scheduler evaluates a dependent change that declares the archived change as a dependency
- **THEN** the dependency is treated as satisfied on the effective base (`integration`)
- **AND** the dependent change becomes eligible for dispatch

#### Scenario: Dependency classification is consistent between analysis and dispatch

- **GIVEN** a change is queued and blocked by a terminal-error dependency
- **WHEN** `classify_queued_work` and `select_changes_for_dispatch` are both called during the same scheduler iteration
- **THEN** both functions classify the dependency target identically
- **AND** the change is excluded from analysis and dispatch without duplication of classification logic

### Requirement: Blocked-only drain excludes pending resolve/reject waiters

内部的な `is_blocked_only_scheduler_state` チェックは blocked-only drain の判定時に executor-local の `resolve_wait_changes` および `reject_wait_changes` が空であることを確認しなければならない（MUST）。これらのセットが空でない場合、blocked-only 判定は `false` を返さなければならない（MUST）。

#### Scenario: resolve wait が存在する場合 blocked-only 判定は false

- **GIVEN** executor-local `resolve_wait_changes` に change `alpha` が存在する
- **AND** `alpha` に対する pending merge task は存在しない（`pending_merge_count == 0`）
- **AND** queued に `alpha` に依存する dependency-blocked な change `beta` が存在する
- **AND** in-flight workspace tasks、manual resolves は存在しない
- **WHEN** `is_blocked_only_scheduler_state` が評価される
- **THEN** `false` が返される
- **AND** スケジューラは終了せず、resolve の dispatch または完了を待つ

#### Scenario: reject wait が存在する場合 blocked-only 判定は false

- **GIVEN** executor-local `reject_wait_changes` に change `alpha` が存在する
- **AND** `alpha` に対する pending merge task は存在しない
- **AND** queued に `alpha` に依存する dependency-blocked な change `beta` が存在する
- **AND** in-flight workspace tasks、manual resolves は存在しない
- **WHEN** `is_blocked_only_scheduler_state` が評価される
- **THEN** `false` が返される

#### Scenario: resolve/reject wait が空で他の条件が blocked-only の場合 true

- **GIVEN** executor-local `resolve_wait_changes` と `reject_wait_changes` が空である
- **AND** in-flight workspace tasks、manual resolves、pending merge tasks が存在しない
- **AND** queued には manual `MergeWait` または dependency-blocked な change のみが存在する
- **WHEN** `is_blocked_only_scheduler_state` が評価される
- **THEN** `true` が返される（既存の blocked-only drain 動作を維持）

### Requirement: Acceptance stalled retry evidence is workspace-local

Ordinary Acceptance retry bookkeeping during an active serial or parallel run MUST remain in memory and MUST NOT use `.cflx/acceptance-state.json` or a worktree checkpoint. Acceptance MUST NOT create an Acceptance-origin `APPLY_BLOCKED/marker.md` or another change-directory artifact.

A validated Acceptance stalled hold MUST be stored in the in-memory `OrchestratorState` only. It MUST NOT be persisted to `~/.local/state/cflx/acceptance-stalls/` or any other out-of-worktree durable location. The in-memory state binds change ID, blocker category, evidence, next action, and resumability for the lifetime of the current process.

In-memory state MAY control ordinary dispatch suppression, stalled presentation, explicit retry eligibility, and Acceptance resume phase. It MUST NOT prove implementation completion, Acceptance PASS, archive readiness, merge eligibility, or base integration. Process restart MUST clear all in-memory stall state. When repository evidence shows a complete unarchived Apply revision, Conflux MUST run Acceptance again and MUST NOT infer PASS.

#### Scenario: stalled hold is process-lifetime only

- **GIVEN** Acceptance records a validated resumable external blocker for a complete Apply revision
- **AND** the managed worktree is clean
- **WHEN** the current Conflux process displays the stalled status
- **THEN** ordinary dispatch starts neither Apply, Acceptance, nor archive
- **AND** the worktree remains clean and the Apply commit remains unchanged
- **AND** no stall file is written under `~/.local/state/cflx/`

#### Scenario: restart clears stall and re-runs acceptance

- **GIVEN** a change was stalled in a previous Conflux process
- **AND** the worktree contains a complete unarchived Apply revision
- **WHEN** a new Conflux process starts and reconciles workspace state
- **THEN** the stalled status is not restored
- **AND** Conflux runs Acceptance again
- **AND** it does not infer prior PASS, enter archive, or rerun Apply solely from missing stall state

#### Scenario: stale stall files are ignored, not consulted or removed

- **GIVEN** files exist under `~/.local/state/cflx/acceptance-stalls/` from a previous version
- **WHEN** a new Conflux process starts and dispatches the same change
- **THEN** no stall file is read and none controls routing
- **AND** the files are left in place so a concurrent older process keeps its own holds
- **AND** no managed worktree is mutated

#### Scenario: explicit retry resumes Acceptance from in-memory hold

- **GIVEN** a valid resumable Acceptance stall exists in the current in-memory state
- **AND** the Apply revision matches
- **WHEN** an operator explicitly retries it
- **THEN** runtime prepares and starts Acceptance without rerunning Apply
- **AND** the in-memory hold is consumed across a successful dispatch-preparation boundary
- **AND** preparation failure retains the blocker evidence and does not dispatch ambiguous work

### Requirement: Runtime acceptance follow-up preserves completed repair work

During apply hydration, runtime MUST preserve the checked state of an existing acceptance finding when its explicit leading bracketed finding code matches the runtime finding, even if descriptive text changed. Findings without an explicit code use their normalized full text as identity. When every existing runtime follow-up task is checked and the normalized finding count is unchanged, runtime MUST preserve all findings as checked even if their text was rewritten. Hydration MUST NOT reopen completed tasks; a subsequent acceptance FAIL routed back to apply remains responsible for recording current findings as new unchecked follow-up work.

#### Scenario: coded finding remains complete after wording changes

- **GIVEN** a runtime follow-up contains checked finding `[RULE_A] old detail`
- **AND** apply hydration receives `[RULE_A] revised detail`
- **WHEN** runtime reconciles the section
- **THEN** the finding remains checked
- **AND** no duplicate unchecked finding is created solely because detail text changed

#### Scenario: fully completed rewritten section remains complete

- **GIVEN** all tasks in the existing runtime follow-up are checked
- **AND** the runtime finding list has the same number of normalized findings
- **WHEN** apply hydration reconciles rewritten finding text
- **THEN** all resulting follow-up tasks remain checked

### Requirement: Acceptance retry safeguards are mode-independent

Serial and parallel execution MUST use equivalent blocker validation, protocol retry, finding normalization, semantic progress, retry, mixed-blocker, stalled persistence, reconciliation, migration, and explicit-retry decisions.

Bare `gated` or legacy `blocked` input MUST share the fixed two-retry protocol bound used for missing verdict while retaining a distinct consecutive counter and corrective context. It MUST NOT consume Apply or explicit-CONTINUE budget, rerun Apply, or persist stalled state. Exhaustion MUST produce a terminal Acceptance protocol error requiring explicit retry.

The existing apply+Acceptance ceiling of ten cycles remains a safety ceiling. A validated repository-external blocker or cycle-exhaustion hold MAY become resumable runtime `stalled` only with explicit evidence; evidence-free exhaustion MUST NOT create a synthetic category or worktree marker.

#### Scenario: bare GATED budget is equivalent across modes

- **GIVEN** serial and parallel Acceptance each emit the same sequence of bare GATED results
- **WHEN** each applies protocol retry policy
- **THEN** both run at most two Acceptance-only retries after the initial result
- **AND** both return the same terminal protocol error on the third consecutive result
- **AND** neither writes stalled state or a worktree marker

#### Scenario: canonical verdict resets bare GATED sequence

- **GIVEN** a bare GATED result was retried
- **WHEN** the next Acceptance invocation returns a canonical PASS, FAIL, CONTINUE, or validated stalled blocker
- **THEN** the consecutive bare-GATED retry counter resets
- **AND** the canonical result follows its normal routing

#### Scenario: equivalent validated blockers produce equivalent state

- **GIVEN** serial and parallel observe equivalent validated structured external blockers for equivalent Apply revisions
- **WHEN** each computes and persists its decision
- **THEN** both preserve the same explicit category, evidence, resumability, next action, and revision binding
- **AND** both enter user-facing `stalled` without dirtying the worktree

### Requirement: Acceptance findings retain repository and external scopes

Runtime MUST classify findings individually as repository-fixable or external/non-mockable. Repository-fixable findings MUST remain actionable Apply repair input. External blockers MUST be retained when repository findings are present, but they MAY enter durable runtime `stalled` only after repository-fixable findings are resolved and the external blocker satisfies the structured validation contract.

Runtime MUST preserve an explicitly supplied supported category and MUST NOT infer credential, infrastructure, or other categories from narrative text. Missing or invalid blocker structure follows bounded protocol error rather than stalled persistence.

#### Scenario: mixed findings preserve both responsibilities

- **GIVEN** Acceptance identifies a repository defect and a concrete external prerequisite
- **WHEN** runtime evaluates the findings
- **THEN** the repository defect remains Apply-repairable
- **AND** the external prerequisite remains non-checkbox blocker metadata
- **AND** runtime does not stall before repository-fixable findings are resolved

#### Scenario: validated external blocker remains after repository repair

- **GIVEN** Apply resolves all repository-fixable findings
- **AND** Acceptance returns a valid structured external blocker
- **WHEN** runtime evaluates the result
- **THEN** it preserves the explicit blocker in revision-bound runtime stalled state
- **AND** it does not create a change-directory marker

#### Scenario: unsupported credential inference is prohibited

- **GIVEN** a bare or incomplete blocker narrative contains words such as credential, token, or auth
- **WHEN** runtime validates the result
- **THEN** it does not assign category `credential` from those words
- **AND** it follows bounded protocol-error handling until a valid structured category and evidence are supplied

### Requirement: Acceptance follow-up rendering uses normalized finding scopes

Serial and parallel execution MUST use the shared normalized finding representation when runtime updates acceptance follow-up state. Repository-fixable findings MUST affect task completion; external blockers MUST remain non-checkbox metadata. Both modes MUST produce equivalent follow-up and prompt context for equivalent observations.

#### Scenario: serial and parallel render equivalent mixed findings

- **GIVEN** serial and parallel receive equivalent repository and external findings
- **WHEN** each persists follow-up and builds the next acceptance context
- **THEN** both produce the same repository task identities
- **AND** both preserve the same external blocker metadata
- **AND** neither replays prior attempt history

#### Scenario: re-reported identity reopens despite detail changes

- **GIVEN** a repository finding was completed in the current follow-up
- **AND** the latest FAIL reports the same stable identity with changed descriptive detail
- **WHEN** runtime updates the section
- **THEN** the finding is reopened as unchecked
- **AND** identities absent from the latest payload are removed

### Requirement: Acceptance finding reconciliation uses stable identity and monotonic completion

Serial and parallel runtime MUST reconcile repository-fixable acceptance findings by stable identity rather than exact human-readable text. Explicit finding codes MUST be preferred when present. When a code is absent, runtime MUST generate a deterministic fallback identity from normalized structural finding fields and MUST NOT require summary or evidence text to remain unchanged.

A completed runtime-owned finding MUST remain completed during apply hydration and reconciliation. Runtime MAY reopen it only while ingesting a new acceptance FAIL payload that explicitly reports the same identity. Serial and parallel execution MUST apply equivalent identity and completion transition rules.

#### Scenario: partial completion survives apply reconciliation

- **GIVEN** a current acceptance follow-up contains multiple findings
- **AND** apply completed one finding while others remain unchecked
- **AND** remediation evidence or human-readable detail changed
- **WHEN** runtime hydrates or reconciles follow-up state during apply
- **THEN** the completed finding remains checked
- **AND** the remaining findings retain their prior state

#### Scenario: latest FAIL explicitly reopens a completed identity

- **GIVEN** a finding is completed in the current follow-up
- **WHEN** a later acceptance FAIL reports the same stable identity with current repository evidence
- **THEN** runtime reopens that finding as unchecked
- **AND** changed summary or evidence does not create a duplicate identity

#### Scenario: missing reviewer code uses runtime fallback identity

- **GIVEN** an acceptance finding has no explicit stable code
- **WHEN** runtime normalizes the finding in serial or parallel execution
- **THEN** both modes derive the same identity from normalized structural fields
- **AND** prose-only changes do not change the identity
- **AND** a distinct rule or repository location does not collide with it

#### Scenario: reconciliation cannot implicitly reopen completion

- **GIVEN** a completed finding exists
- **WHEN** runtime performs any follow-up update outside ingestion of a new FAIL payload
- **THEN** the update cannot transition the finding to unchecked

### Requirement: Acceptance execution creates no JSON checkpoint

Serial and parallel Acceptance execution MUST NOT create, read, update, or delete `.cflx/acceptance-state.json`. Acceptance PASS for an active run MAY be held in memory only until archive handoff. After restart, incomplete archive work MUST be accepted again unless repository evidence already proves archive or base integration.

No out-of-worktree Acceptance stall record exists. In-memory stall state MAY represent a validated temporary external hold bound to the current process lifetime and MUST NOT survive restart.

#### Scenario: uninterrupted pass reaches archive without checkpoint

- **GIVEN** Apply completed and Acceptance runs in the same orchestration process
- **WHEN** Acceptance returns PASS
- **THEN** archive handoff proceeds for that accepted revision
- **AND** neither `.cflx/acceptance-state.json` nor a persisted PASS record exists

#### Scenario: in-memory stall cannot substitute for PASS

- **GIVEN** an in-memory stall state exists
- **WHEN** Conflux evaluates archive readiness
- **THEN** the in-memory state cannot prove PASS or authorize archive
- **AND** Acceptance must pass for the current revision through the normal execution path

#### Scenario: runtime metadata cannot dirty post-archive worktree

- **GIVEN** Acceptance passes and archive artifacts are committed
- **WHEN** post-archive merge verification runs
- **THEN** no Acceptance runtime-state cleanup mutates the managed worktree
- **AND** no manual `MergeWait` is produced solely by runtime stall metadata

#### Scenario: genuine dirty evidence remains a blocker

- **GIVEN** archive artifacts are valid
- **AND** an unrelated user file remains modified
- **WHEN** post-archive merge verification runs
- **THEN** the unrelated dirty worktree remains concrete manual blocker evidence
- **AND** in-memory stall state does not suppress the deferral

### Requirement: Apply completion MUST validate task format before acceptance

After repository task progress appears complete and before acceptance starts, Conflux MUST deterministically validate the worktree-local `tasks.md` task-format contract. A task-format failure MUST keep the change in apply, MUST NOT consume an acceptance attempt, and MUST provide actionable diagnostics to the subsequent apply attempt.

The gate and its retry decision MUST be derived from workspace file state and Git state. It MUST NOT introduce out-of-worktree durable workflow-control state.

#### Scenario: Malformed completed task file stays in apply

**Given**: all implementation checkboxes are complete
**And**: an active task section contains a top-level non-checkbox evidence bullet
**When**: apply evaluates completion
**Then**: Conflux does not invoke acceptance
**And**: the next apply attempt receives the failing file, line, and task-format diagnostic

#### Scenario: Corrected task file proceeds to acceptance

**Given**: a prior pre-accept task-format check failed
**And**: apply corrects the malformed bullet while preserving completed implementation evidence
**When**: worktree-local task-format validation succeeds
**Then**: Conflux proceeds through the existing cleanup and acceptance handoff
**And**: the repair does not consume an extra acceptance attempt

#### Scenario: Restart derives the same pending repair

**Given**: `tasks.md` remains malformed after process restart
**When**: Conflux resumes from the same workspace and Git state
**Then**: it derives the same apply-before-acceptance action and diagnostic from repository state
**And**: deletion of external logs or runtime state does not alter that next action

#### Scenario: Valid completed task file preserves existing handoff

**Given**: implementation checkboxes are complete and task-format validation succeeds
**When**: apply evaluates completion
**Then**: the existing post-apply cleanup and acceptance workflow continues without an additional agent cycle

### Requirement: Acceptance repair state MUST separate actionable payload from retry identity

Serial and parallel runtime MUST keep the complete latest Acceptance finding payload separate from stable retry identities and semantic fingerprints. Updating comparison identities, semantic baselines, cycle counters, or retry checkpoints MUST NOT mutate or replace actionable evidence, required changes, or verification expectations.

Ordinary retry counters and semantic baselines MUST remain in memory. The runtime-owned current follow-up MUST preserve enough immutable actionable finding detail and Apply remediation evidence for an interrupted FAIL-to-Apply handoff using workspace-local evidence. If actionable workspace evidence is absent or invalid after restart, Conflux MUST rerun Acceptance before Apply and MUST NOT infer a repair target, closure, PASS, or archive readiness from hidden state.

#### Scenario: retry checkpoint cannot overwrite payload

- **GIVEN** Acceptance records a detailed finding and runtime derives `repository|path|verification` as comparison identity
- **WHEN** runtime updates retry identity and semantic baseline state
- **THEN** the complete detailed finding remains unchanged
- **AND** the next Apply receives its evidence, required changes, and verification expectations

#### Scenario: restart preserves constitutional routing

- **GIVEN** orchestration stops after FAIL and before repair Apply
- **WHEN** Conflux resumes the workspace
- **THEN** it uses valid workspace-local current finding evidence or reruns Acceptance
- **AND** missing out-of-worktree metadata cannot imply closure or PASS
- **AND** all archive and merge decisions still require repository-verifiable current-revision evidence

### Requirement: Acceptance repair diff MUST cover declared finding work

Before rerunning Acceptance after repair Apply, serial and parallel runtime MUST compare the workspace delta from the finding's FAIL revision through the repair result. For every structured finding, every declared `required_changes` file and every declared `verification` file MUST occur in that delta. Runtime MUST retain actual changed files, uncovered required files, unrelated changed files, and Apply remediation evidence as structured diagnostics.

Passing coverage authorizes only the next Acceptance review; it MUST NOT prove semantic resolution. Missing declared coverage MUST stop before Acceptance with an evidenced, resumable `acceptance_remediation_mismatch` hold. Changes outside the finding contract, including calibration-only or comment-only changes, MUST NOT satisfy missing coverage. Legacy findings without declared path sets MAY retain compatibility behavior.

#### Scenario: complete coverage permits semantic review

- **GIVEN** a structured finding declares an implementation file and a verification file
- **AND** repair Apply changes both files
- **WHEN** runtime validates the repair delta
- **THEN** coverage passes and Acceptance may run
- **AND** runtime does not claim the finding is resolved until Acceptance decides

#### Scenario: calibration-only change stops before Acceptance

- **GIVEN** a finding requires test-support observability and a value-based integration assertion
- **AND** repair Apply changes only a calibration test or unrelated comments
- **WHEN** runtime validates the delta
- **THEN** coverage fails with `acceptance_remediation_mismatch`
- **AND** Acceptance is not invoked
- **AND** diagnostics identify the missing implementation and verification files plus unrelated changes

#### Scenario: unrelated progress cannot satisfy coverage

- **GIVEN** broad semantic fingerprinting observes source, test, or spec changes
- **AND** none covers a finding's declared required file
- **WHEN** runtime evaluates remediation
- **THEN** semantic progress does not override the coverage failure
- **AND** the change enters the same evidenced hold

### Requirement: Repeated Acceptance finding IDs MUST stop automatic repair

Each stable finding ID MUST receive at most one automatic repair Apply after its first FAIL observation. If the next canonical Acceptance FAIL reports the same ID as still open, runtime MUST stop before another Apply with an evidenced, resumable `repeated_acceptance_finding` hold. Unrelated semantic progress, changed prose, changed line numbers, additional evidence, or different representative paths MUST NOT reset that ID's automatic repair budget.

A genuinely new ID receives one automatic repair opportunity. If a FAIL contains both a repeated ID and a new ID, runtime MUST stop atomically, retain every finding in diagnostics, and MUST NOT dispatch partial Apply work. An explicit operator retry MAY start another revision-bound attempt through the existing stalled retry contract, but MUST NOT erase prior occurrence or remediation evidence.

#### Scenario: same ID stops before second repair Apply

- **GIVEN** finding ID `acceptance-secret-value-scan` received one repair Apply
- **AND** the next Acceptance FAIL reports that ID again
- **WHEN** runtime computes the next action
- **THEN** it enters `repeated_acceptance_finding`
- **AND** it does not start a second automatic repair Apply
- **AND** unrelated changed files do not alter the decision

#### Scenario: changed detail does not create a new opportunity

- **GIVEN** a prior finding has a stable ID
- **AND** the next FAIL changes its summary, line numbers, evidence, or cited path while describing the same defect
- **WHEN** runtime reconciles the result
- **THEN** it recognizes the repeated ID
- **AND** it stops automatic repair rather than treating the prose change as progress

#### Scenario: new finding receives one repair opportunity

- **GIVEN** Acceptance no longer reports the prior ID
- **AND** it reports a genuinely new stable ID
- **WHEN** runtime computes the next action
- **THEN** the prior finding is Acceptance-closed
- **AND** the new finding may receive one automatic repair Apply

#### Scenario: mixed repeated and new findings stop atomically

- **GIVEN** a FAIL contains one ID that already consumed its repair opportunity and one new ID
- **WHEN** runtime computes the next action
- **THEN** it starts no Apply
- **AND** diagnostics retain both findings and identify the repeated ID as the stop reason

### Requirement: Acceptance repair-stop diagnostics MUST be actionable and mode-independent

Serial and parallel execution MUST produce equivalent structured diagnostics for `acceptance_remediation_mismatch` and `repeated_acceptance_finding`. Diagnostics MUST include the complete open findings, stable IDs, occurrence counts, relevant FAIL and Apply revisions, declared required and verification files, actual changed files, coverage results, unrelated files and relationship explanations, Apply remediation evidence, stop reason, resumability, and next action.

These temporary hold records MAY control stalled presentation, ordinary dispatch suppression, and explicit retry eligibility only through the revision-bound lifecycle established by `replace-acceptance-marker-stalls`. They MUST NOT prove implementation completion, finding closure, Acceptance PASS, archive readiness, or merge eligibility, and MUST NOT create an Acceptance-origin worktree marker.

#### Scenario: serial and parallel stop with equivalent evidence

- **GIVEN** serial and parallel observe equivalent detailed findings and repair diffs
- **WHEN** each detects remediation mismatch or a repeated ID
- **THEN** both choose the same stop reason and resumability
- **AND** both expose equivalent structured diagnostic fields
- **AND** neither writes Acceptance-origin workflow evidence into the worktree

#### Scenario: explicit retry remains reviewable

- **GIVEN** an operator explicitly retries an evidenced repair hold
- **WHEN** runtime resumes the current revision
- **THEN** prior finding occurrences and remediation diagnostics remain inspectable
- **AND** the retry resumes at the appropriate revision-bound phase
- **AND** runtime still requires a later current-revision Acceptance PASS before archive
