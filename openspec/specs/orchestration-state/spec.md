## Purpose

Provide a single, reducer-owned model for tracking the runtime lifecycle of each change across serial and parallel execution modes. All display status is derived from this shared state; consumers never own an independent lifecycle copy.

## Requirements

### Requirement: Reducer-Owned Change Runtime State

Reducer-owned orchestration state SHALL reflect the latest repository-visible terminal outcome for a change. A recoverable error terminal state SHALL NOT remain the current display state after the same change later emits archive, merge, or resolve success events.

Success events MAY supersede `TerminalState::Error` for the same change because errors from acceptance/apply/archive/resolve attempts are recoverable until the change reaches a repository-visible terminal success or final rejection. Success events MUST NOT overwrite final rejection state.

#### Scenario: archive success clears prior acceptance error in parallel mode

- **GIVEN** change `alpha` is in parallel execution mode
- **AND** `alpha` previously received an `AcceptanceFailed` event
- **WHEN** `alpha` later receives a `ChangeArchived` event
- **THEN** `alpha` SHALL NOT display as `error`
- **AND** parallel-mode archived handling SHALL continue toward merge wait or resolve wait as appropriate

#### Scenario: merge success clears prior recoverable error

- **GIVEN** change `alpha` previously received a recoverable error event
- **AND** `alpha` later archives successfully
- **WHEN** `alpha` receives `MergeCompleted`
- **THEN** `alpha` SHALL have terminal state `Merged`
- **AND** `alpha` SHALL display as `merged`, not `error`

#### Scenario: resolved merge success clears prior recoverable error

- **GIVEN** change `alpha` previously received a recoverable error event
- **WHEN** `alpha` receives `ResolveCompleted`
- **THEN** `alpha` SHALL have terminal state `Merged`
- **AND** `alpha` SHALL display as `merged`, not `error`

#### Scenario: rejected state remains final

- **GIVEN** change `alpha` has terminal state `Rejected`
- **WHEN** stale or unrelated success observations are processed
- **THEN** `alpha` SHALL remain rejected
- **AND** no success event SHALL silently turn rejected into merged or archived

### Requirement: Reducer Input Precedence and Idempotency

Workspace observations SHALL NOT regress a change from terminal `Merged` back to `MergeWait` when the change has already been integrated into the base branch, including fast-forward integration.

#### Scenario: Archived workspace observation does not regress fast-forward merged change

- **GIVEN** a change has already reached terminal `Merged`
- **AND** the integration happened via fast-forward rather than a merge commit
- **WHEN** a later `ChangesRefreshed` event observes the workspace as archived
- **THEN** the reducer keeps the terminal state as `Merged`
- **AND** the derived display status does not regress to `merge wait`

### Requirement: Resolve Wait Queue Ownership

The system SHALL treat `ResolveMerge` / `MergeWait` retry as reducer-owned scheduler intent, not as a TUI-local direct execution operation.

When a merge attempt is deferred because it is auto-resumable, such as another merge or resolve lane currently occupying merge capacity, the reducer SHALL represent the change as `ResolveWait` and keep it eligible for scheduler-owned retry.

When a merge attempt is deferred because manual user action is required, such as a dirty base working tree with uncommitted changes, the reducer SHALL represent the change as `MergeWait` and SHALL remove normal queue intent for that change. Manual merge deferral MUST NOT cause scheduler queue reconciliation to re-dispatch the archived workspace as ordinary queued work.

An explicit `ResolveMerge` command remains the way to retry a manual merge-wait change after the user resolves the manual blocker.

After a change has reached repository-visible base integration, later stale duplicate merge outcomes for the same change MUST NOT regress the reducer-visible lifecycle from `Merged` to `MergeWait` or `ResolveWait`.

#### Scenario: stale archive-incomplete deferral after merge success is ignored

**Given**: change `alpha` has already emitted `MergeCompleted` and is integrated into the base branch
**When**: a stale duplicate post-archive merge path emits `MergeDeferred(auto_resumable=false)` for `alpha` because the archived worktree appears dirty or incomplete during cleanup
**Then**: `alpha` remains terminal `Merged`
**And**: `alpha` is not returned by `queued_change_ids()` or `resolve_wait_change_ids()`
**And**: the user does not see a new manual `MergeWait` blocker for `alpha`

#### Scenario: real dirty-base deferral remains manual merge wait

**Given**: change `beta` is archive-complete but not integrated into the base branch
**And**: the base working tree has uncommitted changes before merge starts
**When**: the merge attempt is deferred with `auto_resumable=false`
**Then**: `beta` displays as `merge wait`
**And**: `beta` is not silently marked merged
**And**: explicit user retry remains required after the dirty-base blocker is resolved

### Requirement: Execution Mode Determines Archive Terminal Semantics

The system SHALL support two execution modes — Serial and Parallel — that determine how `ChangeArchived` events affect terminal state.

In Serial mode, `ChangeArchived` SHALL set the terminal state to `Archived` (a terminal state from which no further transitions occur).

In Parallel mode, `ChangeArchived` SHALL set the wait state to `MergeWait` (a non-terminal state) to allow the subsequent merge step to transition the change to `Merged`.

#### Scenario: Serial mode treats archive as terminal

- **GIVEN** the orchestrator is running in Serial execution mode
- **WHEN** a change receives a `ChangeArchived` event
- **THEN** the terminal state becomes `Archived`
- **AND** the derived display status is `archived`
- **AND** subsequent `MergeCompleted` events for this change are ignored

#### Scenario: Parallel mode treats archive as merge-wait

- **GIVEN** the orchestrator is running in Parallel execution mode
- **WHEN** a change receives a `ChangeArchived` event
- **THEN** the wait state becomes `MergeWait`
- **AND** the terminal state remains `None`
- **AND** the derived display status is `merge wait`

#### Scenario: Parallel mode archive then merge completes lifecycle

- **GIVEN** the orchestrator is running in Parallel execution mode
- **AND** a change has received a `ChangeArchived` event (currently in `MergeWait`)
- **WHEN** a `MergeCompleted` event is received for the change
- **THEN** the terminal state becomes `Merged`
- **AND** the derived display status is `merged`

### Requirement: Parallel Resume Applies Archive-Complete Wait Semantics

In Parallel execution mode, when a resumed workspace is already archive-complete, the shared lifecycle state SHALL apply the same wait semantics as a `ChangeArchived` transition.

This resume-time archive-complete transition MUST preserve the user-visible merge-wait lifecycle and MUST NOT fall back to `not queued` before merge handling has been attempted.

Queue reconciliation MUST NOT redispatch an archive-complete workspace as ordinary queued work while the same change already has an active post-archive merge task or repository-visible base integration.

#### Scenario: active post-archive merge suppresses duplicate archived repair dispatch

**Given**: change `gamma` has an archive-complete workspace
**And**: a post-archive merge task for `gamma` is already active
**When**: scheduler queue reconciliation scans existing worktrees
**Then**: `gamma` is not added again to the ordinary queued dispatch list
**And**: no second merge task is spawned solely from the archived dirty repair candidate path

### Requirement: Resolve Wait Queue Ownership

The system SHALL treat `ResolveMerge` / `MergeWait` retry as reducer-owned scheduler intent, not as a TUI-local direct execution operation.

When a merge attempt is deferred because it is auto-resumable, such as another merge or resolve lane currently occupying merge capacity, the reducer SHALL represent the change as `ResolveWait` and keep it eligible for scheduler-owned retry.

When a merge attempt is deferred because manual user action is required, such as a dirty base working tree with uncommitted changes, the reducer SHALL represent the change as `MergeWait` and SHALL remove normal queue intent for that change. Manual merge deferral MUST NOT cause scheduler queue reconciliation to re-dispatch the archived workspace as ordinary queued work.

An explicit `ResolveMerge` command remains the way to retry a manual merge-wait change after the user resolves the manual blocker.

After a change has reached repository-visible base integration, later stale duplicate merge outcomes for the same change MUST NOT regress the reducer-visible lifecycle from `Merged` to `MergeWait` or `ResolveWait`.

#### Scenario: stale archive-incomplete deferral after merge success is ignored

**Given**: change `alpha` has already emitted `MergeCompleted` and is integrated into the base branch
**When**: a stale duplicate post-archive merge path emits `MergeDeferred(auto_resumable=false)` for `alpha` because the archived worktree appears dirty or incomplete during cleanup
**Then**: `alpha` remains terminal `Merged`
**And**: `alpha` is not returned by `queued_change_ids()` or `resolve_wait_change_ids()`
**And**: the user does not see a new manual `MergeWait` blocker for `alpha`

#### Scenario: real dirty-base deferral remains manual merge wait

**Given**: change `beta` is archive-complete but not integrated into the base branch
**And**: the base working tree has uncommitted changes before merge starts
**When**: the merge attempt is deferred with `auto_resumable=false`
**Then**: `beta` displays as `merge wait`
**And**: `beta` is not silently marked merged
**And**: explicit user retry remains required after the dirty-base blocker is resolved

### Requirement: merge-deferred-reducer-sync

TUI runner の `apply_to_reducer` 条件に `MergeDeferred` イベントを含め、reducer への状態反映を保証する。これにより、次の `ChangesRefreshed` で `apply_display_statuses_from_reducer` が MergeWait を上書きして消す二次バグを防止する。

#### Scenario: merge-deferred-reflected-in-reducer

**Given**: Change A が archive 完了し、merge が dirty base で deferred された
**When**: `MergeDeferred(auto_resumable=false)` イベントが TUI runner で処理される
**Then**: reducer の `apply_execution_event` が呼ばれ、Change A の `WaitState::MergeWait` が設定される

#### Scenario: merge-wait-survives-changes-refreshed

**Given**: Change A が MergeDeferred 経由で reducer に MergeWait が設定されている
**When**: 次の `ChangesRefreshed` イベントが処理される
**Then**: reducer の `display_status()` が "merge wait" を返し、TUI の M キーヒントが表示され続ける

### Requirement: post-archive-merge-dispatch

The system SHALL treat rejection-review wait as reducer-owned scheduler intent, not as TUI-local display state and not as merge/resolve retry intent.

When a rejection review is ready to run but the base-mutating lane is occupied by another active `Resolving` or `Rejecting` change, the reducer SHALL represent the waiting change as `RejectWait` and keep it eligible for scheduler-owned automatic retry. The derived display status SHALL be `reject pending`.

`RejectWait` MUST be distinct from `ResolveWait` so the scheduler can start rejection review, not merge/resolve retry, after the lane clears.

#### Scenario: rejection review waits behind resolving

**Given**: Change A is actively `Resolving`
**And**: Change B produced `openspec/changes/<change_id>/REJECTED.md` and needs dedicated rejecting review
**When**: the scheduler handles B's rejection-review handoff
**Then**: B transitions to `RejectWait`
**And**: B's derived display status is `reject pending`
**And**: B is returned by reducer-owned reject-wait retry membership
**And**: B is not returned by reducer-owned resolve-wait retry membership

#### Scenario: reject pending promotes to rejecting after lane clears

**Given**: Change B is in `RejectWait`
**And**: the active base-mutating lane occupant completes or fails and no other lane occupant remains
**When**: scheduler-owned pending lane retry is evaluated
**Then**: B transitions from `RejectWait` to active `Rejecting`
**And**: B's derived display status becomes `rejecting`
**And**: no other change is active in `Resolving` or `Rejecting`

#### Scenario: rejection review completion clears reject wait intent

**Given**: Change B previously entered `RejectWait`
**And**: B later starts and completes rejection review
**When**: the reducer processes `RejectionReviewCompleted` or `RejectionReviewFailed` for B
**Then**: B is no longer returned by reject-wait retry membership
**And**: B does not regress to `reject pending` on later refresh

### Requirement: OrchestratorState が唯一のループ状態ソースである
`OrchestratorState` はオーケストレーションループの状態（apply 回数、pending/archived/completed 変更セット、イテレーション番号、current change ID）の唯一の正規ソースでなければならない（MUST）。

`Orchestrator` struct および `tui::orchestrator::run_orchestrator` 関数は、これらのカウンタやセットをローカルフィールド/変数として独自に保持してはならない（SHALL NOT）。

状態の参照は `shared_state.read().await` 経由で行い、状態の変更は `apply_execution_event()` または `apply_command()` 経由で行わなければならない（MUST）。

#### Scenario: Orchestrator struct がローカル apply_counts を持たない
- **WHEN** `Orchestrator` struct の定義を確認する
- **THEN** `apply_counts`, `changes_processed`, `iteration`, `current_change_id` フィールドが存在しない
- **AND** これらの値は `self.shared_state.read().await` 経由で取得される

#### Scenario: TUI orchestrator がローカル pending_changes を持たない
- **WHEN** `tui::orchestrator::run_orchestrator` 関数の実装を確認する
- **THEN** `apply_counts`, `pending_changes`, `changes_processed`, `total_changes` のローカル変数が存在しない
- **AND** これらの値は `shared_state.read().await` 経由で取得される

#### Scenario: ステート一貫性の保証
- **WHEN** serial モードでの実行中に Change が archived される
- **THEN** `OrchestratorState` の `pending_changes` が減少する
- **AND** `changes_processed` が増加する
- **AND** 他に同じ情報を保持する変数が更新される必要がない

### Requirement: Reducer-Owned Change Runtime State

Reducer-owned orchestration state SHALL reflect the latest repository-visible terminal outcome for a change. A recoverable error terminal state SHALL NOT remain the current display state after the same change later emits archive, merge, or resolve success events.

Success events MAY supersede `TerminalState::Error` for the same change because errors from acceptance/apply/archive/resolve attempts are recoverable until the change reaches a repository-visible terminal success or final rejection. Success events MUST NOT overwrite final rejection state.

#### Scenario: archive success clears prior acceptance error in parallel mode

- **GIVEN** change `alpha` is in parallel execution mode
- **AND** `alpha` previously received an `AcceptanceFailed` event
- **WHEN** `alpha` later receives a `ChangeArchived` event
- **THEN** `alpha` SHALL NOT display as `error`
- **AND** parallel-mode archived handling SHALL continue toward merge wait or resolve wait as appropriate

#### Scenario: merge success clears prior recoverable error

- **GIVEN** change `alpha` previously received a recoverable error event
- **AND** `alpha` later archives successfully
- **WHEN** `alpha` receives `MergeCompleted`
- **THEN** `alpha` SHALL have terminal state `Merged`
- **AND** `alpha` SHALL display as `merged`, not `error`

#### Scenario: resolved merge success clears prior recoverable error

- **GIVEN** change `alpha` previously received a recoverable error event
- **WHEN** `alpha` receives `ResolveCompleted`
- **THEN** `alpha` SHALL have terminal state `Merged`
- **AND** `alpha` SHALL display as `merged`, not `error`

#### Scenario: rejected state remains final

- **GIVEN** change `alpha` has terminal state `Rejected`
- **WHEN** stale or unrelated success observations are processed
- **THEN** `alpha` SHALL remain rejected
- **AND** no success event SHALL silently turn rejected into merged or archived

### Requirement: Resolve Wait Queue Ownership

The system SHALL treat `ResolveMerge` / `MergeWait` retry as reducer-owned scheduler intent, not as a TUI-local direct execution operation.

When a merge attempt is deferred because it is auto-resumable, such as another merge or resolve lane currently occupying merge capacity, the reducer SHALL represent the change as `ResolveWait` and keep it eligible for scheduler-owned retry.

When a merge attempt is deferred because manual user action is required, such as a dirty base working tree with uncommitted changes, the reducer SHALL represent the change as `MergeWait` and SHALL remove normal queue intent for that change. Manual merge deferral MUST NOT cause scheduler queue reconciliation to re-dispatch the archived workspace as ordinary queued work.

An explicit `ResolveMerge` command remains the way to retry a manual merge-wait change after the user resolves the manual blocker.

After a change has reached repository-visible base integration, later stale duplicate merge outcomes for the same change MUST NOT regress the reducer-visible lifecycle from `Merged` to `MergeWait` or `ResolveWait`.

#### Scenario: stale archive-incomplete deferral after merge success is ignored

**Given**: change `alpha` has already emitted `MergeCompleted` and is integrated into the base branch
**When**: a stale duplicate post-archive merge path emits `MergeDeferred(auto_resumable=false)` for `alpha` because the archived worktree appears dirty or incomplete during cleanup
**Then**: `alpha` remains terminal `Merged`
**And**: `alpha` is not returned by `queued_change_ids()` or `resolve_wait_change_ids()`
**And**: the user does not see a new manual `MergeWait` blocker for `alpha`

#### Scenario: real dirty-base deferral remains manual merge wait

**Given**: change `beta` is archive-complete but not integrated into the base branch
**And**: the base working tree has uncommitted changes before merge starts
**When**: the merge attempt is deferred with `auto_resumable=false`
**Then**: `beta` displays as `merge wait`
**And**: `beta` is not silently marked merged
**And**: explicit user retry remains required after the dirty-base blocker is resolved

### Requirement: Reducer-Owned Change Runtime State

Reducer-owned orchestration state SHALL reflect the latest repository-visible terminal outcome for a change. A recoverable error terminal state SHALL NOT remain the current display state after the same change later emits archive, merge, or resolve success events.

Success events MAY supersede `TerminalState::Error` for the same change because errors from acceptance/apply/archive/resolve attempts are recoverable until the change reaches a repository-visible terminal success or final rejection. Success events MUST NOT overwrite final rejection state.

#### Scenario: archive success clears prior acceptance error in parallel mode

- **GIVEN** change `alpha` is in parallel execution mode
- **AND** `alpha` previously received an `AcceptanceFailed` event
- **WHEN** `alpha` later receives a `ChangeArchived` event
- **THEN** `alpha` SHALL NOT display as `error`
- **AND** parallel-mode archived handling SHALL continue toward merge wait or resolve wait as appropriate

#### Scenario: merge success clears prior recoverable error

- **GIVEN** change `alpha` previously received a recoverable error event
- **AND** `alpha` later archives successfully
- **WHEN** `alpha` receives `MergeCompleted`
- **THEN** `alpha` SHALL have terminal state `Merged`
- **AND** `alpha` SHALL display as `merged`, not `error`

#### Scenario: resolved merge success clears prior recoverable error

- **GIVEN** change `alpha` previously received a recoverable error event
- **WHEN** `alpha` receives `ResolveCompleted`
- **THEN** `alpha` SHALL have terminal state `Merged`
- **AND** `alpha` SHALL display as `merged`, not `error`

#### Scenario: rejected state remains final

- **GIVEN** change `alpha` has terminal state `Rejected`
- **WHEN** stale or unrelated success observations are processed
- **THEN** `alpha` SHALL remain rejected
- **AND** no success event SHALL silently turn rejected into merged or archived

### Requirement: Rejection Flow Execution

The system SHALL execute a rejection flow when acceptance returns a `Blocked` verdict. The rejection flow MUST perform the following steps in order:

1. Extract the rejection reason from acceptance findings
2. Checkout the base branch
3. Generate `openspec/changes/<change_id>/REJECTED.md` containing the rejection reason and timestamp
4. Stage and commit only `openspec/changes/<change_id>/REJECTED.md` on the base branch
5. Delete the rejected worktree

The rejection flow SHALL be used by both serial and parallel execution services.

#### Scenario: Rejection flow commits only REJECTED.md and cleans worktree

- **GIVEN** acceptance has returned `Blocked` for change `fix-auth`
- **WHEN** the rejection flow executes
- **THEN** `openspec/changes/fix-auth/REJECTED.md` is created with the rejection reason
- **AND** the base commit includes only `openspec/changes/fix-auth/REJECTED.md`
- **AND** the worktree for `fix-auth` is deleted

#### Scenario: Rejection flow does not run openspec resolve

- **GIVEN** acceptance has returned `Blocked` for change `fix-auth`
- **WHEN** the rejection flow executes
- **THEN** `openspec resolve fix-auth` is not called
- **AND** rejection completion does not depend on OpenSpec CLI resolve availability

#### Scenario: Rejection flow failure falls back to error state

- **GIVEN** acceptance has returned `Blocked` for a change
- **WHEN** any step of the rejection flow fails (e.g., git commit fails)
- **THEN** the change transitions to `Error` terminal state
- **AND** the worktree is preserved for manual inspection

### Requirement: Rejected Change Exclusion from Change Listing

The system SHALL continue to treat `openspec/changes/<change_id>/REJECTED.md` as the durable rejection marker and exclude marker-bearing changes from the execution-oriented active listing returned by `list_changes_native()`.

This exclusion contract applies to execution candidate discovery and queue addition. It SHALL NOT forbid read-only operational surfaces such as the TUI change list from showing the rejected change as a terminal row.

In addition, when a change transitions into `TerminalState::Rejected`, any frontend-visible execution mark associated with that change SHALL be cleared so the rejected change is not represented as an execution candidate. This clear SHALL restore the UI-visible selection state for that change to `selected = false` while preserving the `rejected` terminal display status.

This execution-mark clear applies only to the rejected change. It MUST NOT clear execution marks for unrelated changes.

#### Scenario: TUI may still show rejected change as read-only row

- **GIVEN** `openspec/changes/fix-auth/REJECTED.md` exists
- **AND** `openspec/changes/fix-auth/proposal.md` exists
- **WHEN** a TUI-facing change snapshot is built
- **THEN** `fix-auth` MAY be included as a read-only rejected row
- **AND** the execution-oriented active listing remains unchanged

#### Scenario: Rejected marker still excludes execution candidate

- **GIVEN** `openspec/changes/fix-auth/REJECTED.md` exists
- **AND** `openspec/changes/fix-auth/proposal.md` exists
- **WHEN** `list_changes_native()` is called for execution candidate discovery
- **THEN** `fix-auth` is NOT included in the returned active change list

#### Scenario: Rejected transition clears execution mark for that change only

- **GIVEN** change `fix-auth` is execution-marked (`selected = true`)
- **AND** another change `add-feature` is also execution-marked
- **WHEN** `fix-auth` transitions into `TerminalState::Rejected`
- **THEN** `fix-auth` is represented as `selected = false`
- **AND** the display status for `fix-auth` remains `rejected`
- **AND** `add-feature` keeps its existing execution mark

#### Scenario: Reactivated rejected change stays unselected after marker removal

- **GIVEN** change `fix-auth` was previously rejected and its execution mark was cleared
- **AND** the user deletes `openspec/changes/fix-auth/REJECTED.md` from the base branch
- **WHEN** `ChangesRefreshed` fires with `fix-auth` present in the active change list
- **THEN** the runtime clears `TerminalState::Rejected` for `fix-auth`
- **AND** the display status for `fix-auth` becomes `not queued`
- **AND** `fix-auth` remains `selected = false` until the user explicitly marks it again

### Requirement: Parallel mode treats archive as merge-wait

- **GIVEN** the orchestrator is running in Parallel execution mode
- **WHEN** a change receives a `ChangeArchived` event
- **THEN** the wait state becomes `MergeWait`
- **AND** the terminal state remains `None`
- **AND** the derived display status is `merge wait`

A parallel archived change MUST leave `MergeWait` as soon as merge handling can proceed automatically. Internal recoverable preconditions such as lazy base-branch initialization MUST NOT keep the change in `MergeWait`; only deferred merge conditions that truly require waiting or user intervention may do so.

#### Scenario: archived change does not stay merge wait for recoverable branch initialization
- **GIVEN** the orchestrator is running in Parallel execution mode
- **AND** a change has received a `ChangeArchived` event
- **AND** merge handling discovers that the Git base branch has not yet been cached
- **WHEN** the system can initialize that base branch from repository state
- **THEN** the change proceeds through merge handling
- **AND** the reducer does not preserve `merge wait` solely because of the missing cached branch name

#### Scenario: archived change enters error instead of merge wait on unrecoverable branch discovery failure
- **GIVEN** the orchestrator is running in Parallel execution mode
- **AND** a change has received a `ChangeArchived` event
- **AND** merge handling cannot determine the base branch because the repository is detached HEAD
- **WHEN** the failure is reported
- **THEN** the change is treated as an execution error
- **AND** the reducer does not classify the failure as `merge wait`

### Requirement: Rejection Flow Execution

The system SHALL execute a rejection flow when acceptance returns a `Gated` verdict, including compatibility inputs where legacy `Blocked` verdicts are parsed as acceptance-gated outcomes. Apply execution MAY generate `openspec/changes/<change_id>/REJECTED.md` as a rejection proposal when it encounters an implementation blocker that prevents completion. This proposal file SHALL NOT become a terminal rejection by itself. Acceptance SHALL review the blocker and decide whether to confirm the rejection. Only after acceptance confirms the gated verdict SHALL the runtime treat the change as rejected, commit only `REJECTED.md` on the base branch, and delete the worktree.

#### Scenario: apply-generated rejection proposal requires acceptance confirmation

- **GIVEN** apply execution writes `openspec/changes/fix-auth/REJECTED.md` because of an implementation blocker
- **WHEN** acceptance has not yet confirmed the gated verdict
- **THEN** the change is not yet in `Rejected` terminal state
- **AND** no rejection flow commit is created on the base branch

#### Scenario: acceptance-confirmed apply blocker transitions to rejected terminal state

- **GIVEN** apply execution has generated `openspec/changes/fix-auth/REJECTED.md`
- **AND** acceptance confirms the gated verdict
- **WHEN** the rejection flow completes
- **THEN** the terminal state becomes `Rejected` with the rejection reason
- **AND** the derived display status is `rejected`
- **AND** the change cannot be re-queued via `AddToQueue`

### Requirement: Rejection Flow Execution

The system SHALL execute a rejection flow when acceptance returns a `Gated` verdict, including compatibility inputs where legacy `Blocked` verdicts are parsed as acceptance-gated outcomes. The rejection flow MUST write and commit only `openspec/changes/<change_id>/REJECTED.md` on the base branch. The rejection flow MUST NOT stage, merge, or commit any other files from the rejected worktree, including proposal, tasks, spec deltas, or product code changes. The runtime SHALL treat the `REJECTED.md` marker commit itself as the durable rejection record and SHALL NOT require `openspec resolve <change_id>` as part of the rejection flow.

#### Scenario: rejection flow commits only REJECTED marker

- **GIVEN** acceptance confirms a gated verdict for `fix-auth`
- **WHEN** the rejection flow executes
- **THEN** the base branch commit includes `openspec/changes/fix-auth/REJECTED.md`
- **AND** no other files from the rejected worktree are staged or committed

#### Scenario: rejection flow does not invoke openspec resolve

- **GIVEN** acceptance confirms a gated verdict for `fix-auth`
- **WHEN** the rejection flow executes
- **THEN** `openspec resolve fix-auth` is not invoked
- **AND** rejection completion does not depend on OpenSpec CLI availability

#### Scenario: worktree cleanup occurs after reject marker commit

- **GIVEN** the rejection flow has committed `openspec/changes/fix-auth/REJECTED.md` on the base branch
- **WHEN** the flow completes
- **THEN** the rejected worktree is cleaned up
- **AND** the rejected change remains represented by the base-side `REJECTED.md` marker

## Requirements

### Requirement: Force stop and dequeue returns a running change to not queued

The system SHALL support a force-stop-and-dequeue operation for a running change.

This operation MUST cancel the in-flight execution for the target change and, once cancellation is confirmed, clear the reducer-owned runtime state back to a non-terminal idle queue-off state.

After the operation completes, the target change MUST satisfy all of the following:

- `queue_intent` is `NotQueued`
- `activity` is `Idle`
- `wait_state` is `None`
- `terminal` is `None`
- the derived display status is `not queued`

The force-stop-and-dequeue operation MUST be distinct from terminal stop semantics such as `Stopped`, and MUST NOT leave the change in a terminal stopped state.

#### Scenario: Running apply is force-stopped and dequeued

- **GIVEN** a change is currently in an active execution stage such as `Applying`
- **WHEN** the user invokes force-stop-and-dequeue for that change
- **THEN** the in-flight execution is cancelled
- **AND** after cancellation confirmation the reducer clears the change to `NotQueued` + `Idle` + `None wait` + `None terminal`
- **AND** the derived display status is `not queued`

#### Scenario: Stale stop completion does not create terminal stopped state

- **GIVEN** a running change has already completed force-stop-and-dequeue
- **WHEN** a late stop-related event from the cancelled worker arrives
- **THEN** the reducer ignores any regression to terminal `Stopped`
- **AND** the derived display status remains `not queued`

### Requirement: Force-stop-and-dequeue does not auto-resume work

After a change has been force-stopped and dequeued, the system SHALL NOT automatically re-queue or restart that change unless the user explicitly requests queueing again.

#### Scenario: Refresh does not re-queue dequeued change

- **GIVEN** a change has completed force-stop-and-dequeue and currently displays `not queued`
- **WHEN** the system processes a later refresh or reconciliation pass
- **THEN** the reducer preserves `NotQueued`
- **AND** the change does not transition back to an active or queued state without a new explicit queue command

### Requirement: Force-stop-and-dequeue only applies to retryable active work

The system SHALL apply force-stop-and-dequeue only to changes that are currently retryable and in-flight or queued for in-flight cancellation handling.

The operation MUST NOT convert permanent terminal changes such as `Archived`, `Merged`, or `Rejected` into `not queued`.

#### Scenario: Archived change ignores force-stop-and-dequeue

- **GIVEN** a change is already in terminal `Archived`
- **WHEN** force-stop-and-dequeue is requested for that change
- **THEN** the reducer treats the request as a no-op
- **AND** the derived display status remains `archived`

### Requirement: Reducer-Owned Change Runtime State

Reducer-owned orchestration state SHALL reflect the latest repository-visible terminal outcome for a change. A recoverable error terminal state SHALL NOT remain the current display state after the same change later emits archive, merge, or resolve success events.

Success events MAY supersede `TerminalState::Error` for the same change because errors from acceptance/apply/archive/resolve attempts are recoverable until the change reaches a repository-visible terminal success or final rejection. Success events MUST NOT overwrite final rejection state.

#### Scenario: archive success clears prior acceptance error in parallel mode

- **GIVEN** change `alpha` is in parallel execution mode
- **AND** `alpha` previously received an `AcceptanceFailed` event
- **WHEN** `alpha` later receives a `ChangeArchived` event
- **THEN** `alpha` SHALL NOT display as `error`
- **AND** parallel-mode archived handling SHALL continue toward merge wait or resolve wait as appropriate

#### Scenario: merge success clears prior recoverable error

- **GIVEN** change `alpha` previously received a recoverable error event
- **AND** `alpha` later archives successfully
- **WHEN** `alpha` receives `MergeCompleted`
- **THEN** `alpha` SHALL have terminal state `Merged`
- **AND** `alpha` SHALL display as `merged`, not `error`

#### Scenario: resolved merge success clears prior recoverable error

- **GIVEN** change `alpha` previously received a recoverable error event
- **WHEN** `alpha` receives `ResolveCompleted`
- **THEN** `alpha` SHALL have terminal state `Merged`
- **AND** `alpha` SHALL display as `merged`, not `error`

#### Scenario: rejected state remains final

- **GIVEN** change `alpha` has terminal state `Rejected`
- **WHEN** stale or unrelated success observations are processed
- **THEN** `alpha` SHALL remain rejected
- **AND** no success event SHALL silently turn rejected into merged or archived

### Requirement: Rejected terminal state remains distinct from errors

The terminal result MUST include `Rejected` as a permanent terminal state distinct from `Error`. A rejected change is one where rejecting review has confirmed the specification is unimplementable or otherwise out of scope for completion, requiring a rollback to the base branch with a documented reason.

Acceptance-gate and rejecting-review holds that are not confirmed as rejected MUST remain non-terminal and display as `stalled` when execution is paused for intervention.

#### Scenario: rejecting-confirmed change becomes rejected terminal state

- **GIVEN** a change is in `Rejecting`
- **AND** the rejection flow completes (`REJECTED.md` committed and worktree removed)
- **WHEN** the reducer applies the terminal rejection event
- **THEN** the terminal result becomes `Rejected`
- **AND** the derived display status is `rejected`

#### Scenario: unconfirmed acceptance hold remains stalled

- **GIVEN** acceptance reports an implementation blocker
- **AND** rejecting review has not confirmed terminal rejection
- **WHEN** the reducer exposes the paused lifecycle state
- **THEN** the terminal result remains `None`
- **AND** the derived display status is `stalled`

### Requirement: Rejection proposal dismissal returns to apply with recovery tasks

When rejecting review dismisses a worktree-local `openspec/changes/<change_id>/REJECTED.md` proposal, the runtime SHALL return the change to active apply rather than terminal rejection.

Before returning to apply, the runtime SHALL remove the worktree-local `REJECTED.md` file and ensure `openspec/changes/<change_id>/tasks.md` contains at least one unchecked task describing a non-rejection recovery step. The runtime SHALL route this dismiss path directly back to `Applying` rather than through the normal acceptance stage.

#### Scenario: dismissing rejection proposal resumes apply

- **GIVEN** a change is currently in `Rejecting`
- **AND** the worktree contains `openspec/changes/fix-auth/REJECTED.md`
- **WHEN** rejecting review dismisses the reject proposal
- **THEN** the worktree-local `REJECTED.md` is removed
- **AND** `openspec/changes/fix-auth/tasks.md` is updated with at least one unchecked recovery task that is not a reject action
- **AND** the active execution stage becomes `Applying`
- **AND** the derived display status is `applying`

### Requirement: Reducer-Owned Change Runtime State

Reducer-owned orchestration state SHALL reflect the latest repository-visible terminal outcome for a change. A recoverable error terminal state SHALL NOT remain the current display state after the same change later emits archive, merge, or resolve success events.

Success events MAY supersede `TerminalState::Error` for the same change because errors from acceptance/apply/archive/resolve attempts are recoverable until the change reaches a repository-visible terminal success or final rejection. Success events MUST NOT overwrite final rejection state.

#### Scenario: archive success clears prior acceptance error in parallel mode

- **GIVEN** change `alpha` is in parallel execution mode
- **AND** `alpha` previously received an `AcceptanceFailed` event
- **WHEN** `alpha` later receives a `ChangeArchived` event
- **THEN** `alpha` SHALL NOT display as `error`
- **AND** parallel-mode archived handling SHALL continue toward merge wait or resolve wait as appropriate

#### Scenario: merge success clears prior recoverable error

- **GIVEN** change `alpha` previously received a recoverable error event
- **AND** `alpha` later archives successfully
- **WHEN** `alpha` receives `MergeCompleted`
- **THEN** `alpha` SHALL have terminal state `Merged`
- **AND** `alpha` SHALL display as `merged`, not `error`

#### Scenario: resolved merge success clears prior recoverable error

- **GIVEN** change `alpha` previously received a recoverable error event
- **WHEN** `alpha` receives `ResolveCompleted`
- **THEN** `alpha` SHALL have terminal state `Merged`
- **AND** `alpha` SHALL display as `merged`, not `error`

#### Scenario: rejected state remains final

- **GIVEN** change `alpha` has terminal state `Rejected`
- **WHEN** stale or unrelated success observations are processed
- **THEN** `alpha` SHALL remain rejected
- **AND** no success event SHALL silently turn rejected into merged or archived

### Requirement: Reducer Input Precedence and Idempotency

Workspace observations SHALL NOT regress a change from terminal `Merged` back to `MergeWait` when the change has already been integrated into the base branch, including fast-forward integration.

#### Scenario: Archived workspace observation does not regress fast-forward merged change

- **GIVEN** a change has already reached terminal `Merged`
- **AND** the integration happened via fast-forward rather than a merge commit
- **WHEN** a later `ChangesRefreshed` event observes the workspace as archived
- **THEN** the reducer keeps the terminal state as `Merged`
- **AND** the derived display status does not regress to `merge wait`

### Requirement: Reducer-Owned Change Runtime State

Reducer-owned orchestration state SHALL reflect the latest repository-visible terminal outcome for a change. A recoverable error terminal state SHALL NOT remain the current display state after the same change later emits archive, merge, or resolve success events.

Success events MAY supersede `TerminalState::Error` for the same change because errors from acceptance/apply/archive/resolve attempts are recoverable until the change reaches a repository-visible terminal success or final rejection. Success events MUST NOT overwrite final rejection state.

#### Scenario: archive success clears prior acceptance error in parallel mode

- **GIVEN** change `alpha` is in parallel execution mode
- **AND** `alpha` previously received an `AcceptanceFailed` event
- **WHEN** `alpha` later receives a `ChangeArchived` event
- **THEN** `alpha` SHALL NOT display as `error`
- **AND** parallel-mode archived handling SHALL continue toward merge wait or resolve wait as appropriate

#### Scenario: merge success clears prior recoverable error

- **GIVEN** change `alpha` previously received a recoverable error event
- **AND** `alpha` later archives successfully
- **WHEN** `alpha` receives `MergeCompleted`
- **THEN** `alpha` SHALL have terminal state `Merged`
- **AND** `alpha` SHALL display as `merged`, not `error`

#### Scenario: resolved merge success clears prior recoverable error

- **GIVEN** change `alpha` previously received a recoverable error event
- **WHEN** `alpha` receives `ResolveCompleted`
- **THEN** `alpha` SHALL have terminal state `Merged`
- **AND** `alpha` SHALL display as `merged`, not `error`

#### Scenario: rejected state remains final

- **GIVEN** change `alpha` has terminal state `Rejected`
- **WHEN** stale or unrelated success observations are processed
- **THEN** `alpha` SHALL remain rejected
- **AND** no success event SHALL silently turn rejected into merged or archived

### Requirement: WebSocket change status consistency with TUI

Server-mode WebSocket API SHALL produce the same set of display status strings as `ChangeRuntimeState.display_status()`. The system MUST NOT maintain a separate mapping from workspace states to display strings that diverges from the reducer-derived status vocabulary.

#### Scenario: All display statuses are representable in WebSocket payloads

- **GIVEN** the reducer can produce any of: `not queued`, `queued`, `blocked`, `applying`, `accepting`, `rejecting`, `archiving`, `resolving`, `merge wait`, `resolve pending`, `archived`, `merged`, `rejected`, `error`, `stopped`
- **WHEN** a WebSocket client receives a change list
- **THEN** the status field for each change is one of the above values

### Requirement: Scheduler dispatch derives queued candidates from reducer state

The parallel scheduler's decision to dispatch queued changes SHALL be derived from reducer-observable state (queue intent, active execution stage, available slots) rather than transient event flags. This ensures that changes with `QueueIntent::Queued` in the reducer are always considered for dispatch when execution capacity exists.

The scheduler SHALL reconcile reducer-visible queued intent into its scheduler-local candidate set before declaring the local queue empty, before exiting due to drained work, and before skipping re-analysis solely because the local queue is empty. Dynamic queue notifications MAY wake the scheduler, but they MUST NOT be the only mechanism by which reducer-queued work becomes eligible for analysis.

#### Scenario: Reducer queued change is visible to scheduler dispatch

- **GIVEN** a change has `QueueIntent::Queued` in the reducer
- **AND** no activity stage is active for that change
- **AND** available execution slots are greater than zero
- **WHEN** the scheduler evaluates dispatch candidates
- **THEN** the change is included in the re-analysis candidate set
- **AND** the scheduler does not require a separate event flag to consider this change

#### Scenario: Local queued vector cannot hide reducer queued intent

- **GIVEN** a change has `QueueIntent::Queued` in the reducer
- **AND** the scheduler-local queued vector does not include that change
- **AND** the change is loadable from active OpenSpec change state
- **AND** no terminal or active state makes the change ineligible
- **WHEN** the scheduler evaluates whether work is drained or analysis should run
- **THEN** the scheduler reconciles the change into its local candidate set
- **AND** the scheduler does not exit or sleep indefinitely solely because its pre-reconcile local queued vector was empty

#### Scenario: Queued intent skip reason is observable

- **GIVEN** a change has `QueueIntent::Queued` in the reducer
- **AND** scheduler reconciliation does not add it to analysis candidates
- **WHEN** the scheduler records the reconciliation result
- **THEN** the reason is observable as a log or event
- **AND** the reason distinguishes at least active/in-flight, terminal, not loadable, no available slots, and debounce-delayed cases when those cases apply

### Requirement: post-archive-merge-dispatch

The system SHALL treat rejection-review wait as reducer-owned scheduler intent, not as TUI-local display state and not as merge/resolve retry intent.

When a rejection review is ready to run but the base-mutating lane is occupied by another active `Resolving` or `Rejecting` change, the reducer SHALL represent the waiting change as `RejectWait` and keep it eligible for scheduler-owned automatic retry. The derived display status SHALL be `reject pending`.

`RejectWait` MUST be distinct from `ResolveWait` so the scheduler can start rejection review, not merge/resolve retry, after the lane clears.

#### Scenario: rejection review waits behind resolving

**Given**: Change A is actively `Resolving`
**And**: Change B produced `openspec/changes/<change_id>/REJECTED.md` and needs dedicated rejecting review
**When**: the scheduler handles B's rejection-review handoff
**Then**: B transitions to `RejectWait`
**And**: B's derived display status is `reject pending`
**And**: B is returned by reducer-owned reject-wait retry membership
**And**: B is not returned by reducer-owned resolve-wait retry membership

#### Scenario: reject pending promotes to rejecting after lane clears

**Given**: Change B is in `RejectWait`
**And**: the active base-mutating lane occupant completes or fails and no other lane occupant remains
**When**: scheduler-owned pending lane retry is evaluated
**Then**: B transitions from `RejectWait` to active `Rejecting`
**And**: B's derived display status becomes `rejecting`
**And**: no other change is active in `Resolving` or `Rejecting`

#### Scenario: rejection review completion clears reject wait intent

**Given**: Change B previously entered `RejectWait`
**And**: B later starts and completes rejection review
**When**: the reducer processes `RejectionReviewCompleted` or `RejectionReviewFailed` for B
**Then**: B is no longer returned by reject-wait retry membership
**And**: B does not regress to `reject pending` on later refresh

### Requirement: Rejected Change Exclusion from Change Listing

The system SHALL continue to treat `openspec/changes/<change_id>/REJECTED.md` as the durable rejection marker and exclude marker-bearing changes from the execution-oriented active listing returned by `list_changes_native()`.

This exclusion contract applies to execution candidate discovery and queue addition. It SHALL NOT forbid read-only operational surfaces such as the TUI change list from showing the rejected change as a terminal row.

#### Scenario: TUI may still show rejected change as read-only row

- **GIVEN** `openspec/changes/fix-auth/REJECTED.md` exists
- **AND** `openspec/changes/fix-auth/proposal.md` exists
- **WHEN** a TUI-facing change snapshot is built
- **THEN** `fix-auth` MAY be included as a read-only rejected row
- **AND** the execution-oriented active listing remains unchanged

#### Scenario: Rejected marker still excludes execution candidate

- **GIVEN** `openspec/changes/fix-auth/REJECTED.md` exists
- **AND** `openspec/changes/fix-auth/proposal.md` exists
- **WHEN** `list_changes_native()` is called for execution candidate discovery
- **THEN** `fix-auth` is NOT included in the returned active change list

In addition, when a change transitions into `TerminalState::Rejected`, any frontend-visible execution mark associated with that change SHALL be cleared so the rejected change is not represented as an execution candidate. This clear SHALL restore the UI-visible selection state for that change to `selected = false` while preserving the `rejected` terminal display status.

This execution-mark clear applies only to the rejected change. It MUST NOT clear execution marks for unrelated changes.

#### Scenario: Rejected transition clears execution mark for that change only

- **GIVEN** change `fix-auth` is execution-marked (`selected = true`)
- **AND** another change `add-feature` is also execution-marked
- **WHEN** `fix-auth` transitions into `TerminalState::Rejected`
- **THEN** `fix-auth` is represented as `selected = false`
- **AND** the display status for `fix-auth` remains `rejected`
- **AND** `add-feature` keeps its existing execution mark

#### Scenario: Reactivated rejected change stays unselected after marker removal

- **GIVEN** change `fix-auth` was previously rejected and its execution mark was cleared
- **AND** the user deletes `openspec/changes/fix-auth/REJECTED.md` from the base branch
- **WHEN** `ChangesRefreshed` fires with `fix-auth` present in the active change list
- **THEN** the runtime clears `TerminalState::Rejected` for `fix-auth`
- **AND** the display status for `fix-auth` becomes `not queued`
- **AND** `fix-auth` remains `selected = false` until the user explicitly marks it again

### Requirement: Resolve Wait Queue Ownership

The system SHALL treat `ResolveMerge` / `MergeWait` retry as reducer-owned scheduler intent, not as a TUI-local direct execution operation.

When a merge attempt is deferred because it is auto-resumable, such as another merge or resolve lane currently occupying merge capacity, the reducer SHALL represent the change as `ResolveWait` and keep it eligible for scheduler-owned retry.

When a merge attempt is deferred because manual user action is required, such as a dirty base working tree with uncommitted changes, the reducer SHALL represent the change as `MergeWait` and SHALL remove normal queue intent for that change. Manual merge deferral MUST NOT cause scheduler queue reconciliation to re-dispatch the archived workspace as ordinary queued work.

An explicit `ResolveMerge` command remains the way to retry a manual merge-wait change after the user resolves the manual blocker.

After a change has reached repository-visible base integration, later stale duplicate merge outcomes for the same change MUST NOT regress the reducer-visible lifecycle from `Merged` to `MergeWait` or `ResolveWait`.

#### Scenario: stale archive-incomplete deferral after merge success is ignored

**Given**: change `alpha` has already emitted `MergeCompleted` and is integrated into the base branch
**When**: a stale duplicate post-archive merge path emits `MergeDeferred(auto_resumable=false)` for `alpha` because the archived worktree appears dirty or incomplete during cleanup
**Then**: `alpha` remains terminal `Merged`
**And**: `alpha` is not returned by `queued_change_ids()` or `resolve_wait_change_ids()`
**And**: the user does not see a new manual `MergeWait` blocker for `alpha`

#### Scenario: real dirty-base deferral remains manual merge wait

**Given**: change `beta` is archive-complete but not integrated into the base branch
**And**: the base working tree has uncommitted changes before merge starts
**When**: the merge attempt is deferred with `auto_resumable=false`
**Then**: `beta` displays as `merge wait`
**And**: `beta` is not silently marked merged
**And**: explicit user retry remains required after the dirty-base blocker is resolved

### Requirement: post-archive-status-idempotency

Parallel post-archive status updates SHALL be idempotent and monotonic with respect to final merge completion. Once a change reaches `Merged`, later archive milestones, workspace refreshes, cleanup events, or archived workspace observations MUST NOT regress its derived display status to `archived`, `merge wait`, `resolve pending`, or `resolving`.

#### Scenario: merged does not oscillate with archived

**Given**: Change B has reached terminal `Merged` in parallel mode
**When**: a later `ChangeArchived` event, `ChangesRefreshed` event, archived workspace observation, or cleanup event is processed for B
**Then**: B SHALL remain terminal `Merged`
**And**: B's derived display status SHALL remain `merged`
**And**: the UI SHALL NOT alternate B between `merged` and `archived`

#### Scenario: no-blocker merge wait does not oscillate before merged

**Given**: no other merge/resolve lane is actively occupied
**And**: Change B has just been archived in parallel mode
**And**: no manual merge blocker has been detected
**When**: post-archive events and refreshes are processed before merge completion
**Then**: B SHALL NOT display `merge wait`
**And**: B SHALL either display active merge handling as `resolving` or final completion as `merged`

### Requirement: Archived dirty workspaces remain scheduler-recoverable after archive finalization failure

When a parallel workspace has already moved a change into `openspec/changes/archive/` but the archive commit is still incomplete, the runtime SHALL treat that state as recoverable scheduler-owned work rather than as permanently terminal solely because a prior run emitted an archive failure.

Recovery decisions SHALL be derived from repository-visible workspace state, including active change path absence, archive path presence, incomplete archive commit verification, and current git state. The system MUST NOT require durable external resume state to rediscover the workspace.

The scheduler SHALL be able to re-own and resume archive finalization repair for such an archived dirty workspace on a later cycle or restarted run, unless the bounded recovery policy has been exhausted for the current attempted repair path and the workspace is explicitly classified as terminal.

#### Scenario: archived dirty workspace is reclaimed on later scheduler cycle

- **GIVEN** change `alpha` has been moved to `openspec/changes/archive/2026-05-08-alpha/` in its workspace
- **AND** `openspec/changes/alpha/` no longer exists in that workspace
- **AND** the workspace still lacks a complete `Archive: alpha` commit
- **WHEN** a later scheduler cycle inspects repository-visible workspace state
- **THEN** Conflux reclaims `alpha` as archive-finalization recovery work
- **AND** the scheduler does not remain idle while that recoverable work exists

#### Scenario: archived dirty recovery does not require full archive command rerun

- **GIVEN** archive file movement for `alpha` is already correct
- **AND** only archive commit finalization remains incomplete
- **WHEN** Conflux resumes recovery for `alpha`
- **THEN** it resumes archive finalization repair rather than re-running the full archive command unnecessarily
- **AND** it still verifies that archive file-state has not regressed

#### Scenario: archive move regression re-enters full archive path

- **GIVEN** a previously archived dirty workspace for `alpha`
- **AND** later inspection shows the archive entry is missing or the active change directory has reappeared
- **WHEN** Conflux evaluates recovery
- **THEN** it does not treat the workspace as archive-finalization-only recovery
- **AND** it may require the broader archive path again based on current file state

#### Scenario: archived dirty state is distinct from terminal archive failure

- **GIVEN** a run previously emitted `Archive commit verification failed` for `alpha`
- **AND** the workspace still shows archive files present and commit incomplete
- **WHEN** Conflux derives current runtime state from the workspace
- **THEN** it exposes a recoverable archived-dirty/archive-finalization-needed state instead of only terminal archive failure
- **AND** user-visible events/logs distinguish that recoverable state from exhausted terminal failure

#### Scenario: exhausted archive-finalization recovery becomes terminal

- **GIVEN** archived dirty recovery for `alpha` has exhausted its bounded retry policy
- **WHEN** Conflux reports the final outcome
- **THEN** it MAY emit a terminal archive failure
- **AND** the reported blocker identifies the final archive-finalization reason rather than implying the archive move itself never happened
