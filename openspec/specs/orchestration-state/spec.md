## Purpose

Provide a single, reducer-owned model for tracking the runtime lifecycle of each change across serial and parallel execution modes. All display status is derived from this shared state; consumers never own an independent lifecycle copy.

## Requirements

### Requirement: Reducer-Owned Change Runtime State

The runtime state MUST distinguish at least the following blocker-adjacent concerns without collapsing them into a single `blocked` label:

- dependency wait reason (`blocked`, canonical concept: `dependency-blocked`)
- apply/rejecting resumable hold (`stalled`)
- acceptance gate observation (`gated`, canonical concept: `acceptance-gated`)

Derived display status exposed from reducer-owned runtime state SHALL preserve this distinction for consumers.

#### Scenario: dependency wait remains blocked
- **GIVEN** a change is queued for execution
- **AND** dependency analysis reports unresolved dependencies
- **WHEN** the reducer applies the dependency-blocked input
- **THEN** the wait reason becomes dependency `blocked`
- **AND** the derived display status is `blocked`
- **AND** the change is not labeled `stalled` or `gated`

#### Scenario: resumable apply hold is stalled
- **GIVEN** apply or rejecting review reports a resumable blocker for a change
- **WHEN** the reducer applies the hold transition
- **THEN** the lifecycle state becomes `stalled`
- **AND** terminal result remains `None`
- **AND** the derived display status is `stalled`
- **AND** the reducer does not reuse dependency `blocked` for this hold

#### Scenario: acceptance gate is exposed separately
- **GIVEN** acceptance reports a blocker observation before the next lifecycle step is chosen
- **WHEN** reducer-owned state or its derived event/status surface exposes that observation
- **THEN** the displayed wording is `gated`
- **AND** the canonical taxonomy identifies the observation as `acceptance-gated`
- **AND** the observation is distinguishable from dependency `blocked` and apply `stalled`

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

When the scheduler retries an archived Git merge and the merge path reaches a normal merge-ready state without unresolved conflicts, the runtime SHALL complete that merge through the normal merge/verification path and SHALL NOT start AI conflict resolution solely because the retry entered the resolve-capable code path.

Post-merge verification for this path SHALL accept repository-visible merge success without requiring the archived source branch tip to continue containing the pre-merge base after the merge commit has already integrated the change into the target branch.

Reducer-owned `ResolveWait` SHALL be considered schedulable work even when there are no queued active changes. Scheduler startup and idle/drained checks MUST include this reducer-owned work before deciding that a run has no work.

#### Scenario: reducer-owned resolve wait survives empty startup

**Given**: change `alpha` is stored in the shared reducer as `ResolveWait`
**And**: the scheduler starts with an empty active change list
**When**: the scheduler evaluates whether work is drained
**Then**: it treats `alpha` as pending scheduler-owned retry work
**And**: it does not emit only a zero-change completion without attempting the retry

#### Scenario: resolve wait is synchronized before drained exit

**Given**: shared reducer state contains one or more `ResolveWait` changes
**When**: the scheduler loop begins an iteration
**Then**: it synchronizes those IDs into executor retry state before checking whether queued, in-flight, resolve-wait, manual-resolve, and pending-merge work are all empty

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

#### Scenario: Resume-time archived change becomes merge wait

- **GIVEN** the orchestrator is running in Parallel execution mode
- **AND** a reused workspace is detected as already archived but not yet merged
- **WHEN** the parallel resume path reports archive-complete completion for that change
- **THEN** the wait state becomes `MergeWait`
- **AND** the derived display status is merge wait
- **AND** the change does not regress to `not queued` during the restart flow


#

### Requirement: Resolve Wait Queue Ownership

The system SHALL treat `ResolveMerge` / `MergeWait` retry as reducer-owned scheduler intent, not as a TUI-local direct execution operation.

When the scheduler retries an archived Git merge and the merge path reaches a normal merge-ready state without unresolved conflicts, the runtime SHALL complete that merge through the normal merge/verification path and SHALL NOT start AI conflict resolution solely because the retry entered the resolve-capable code path.

Post-merge verification for this path SHALL accept repository-visible merge success without requiring the archived source branch tip to continue containing the pre-merge base after the merge commit has already integrated the change into the target branch.

Reducer-owned `ResolveWait` SHALL be considered schedulable work even when there are no queued active changes. Scheduler startup and idle/drained checks MUST include this reducer-owned work before deciding that a run has no work.

#### Scenario: reducer-owned resolve wait survives empty startup

**Given**: change `alpha` is stored in the shared reducer as `ResolveWait`
**And**: the scheduler starts with an empty active change list
**When**: the scheduler evaluates whether work is drained
**Then**: it treats `alpha` as pending scheduler-owned retry work
**And**: it does not emit only a zero-change completion without attempting the retry

#### Scenario: resolve wait is synchronized before drained exit

**Given**: shared reducer state contains one or more `ResolveWait` changes
**When**: the scheduler loop begins an iteration
**Then**: it synchronizes those IDs into executor retry state before checking whether queued, in-flight, resolve-wait, manual-resolve, and pending-merge work are all empty

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

When a change is archived in parallel mode, the orchestrator must attempt to merge immediately unless another non-terminal change is actively occupying the automatic retry blocker lane. The only lifecycle activities that occupy that lane are `Resolving` and `Rejecting` on another change.

Automatic `ResolveWait` / `resolve pending` MUST NOT be created solely because another change is `Applying`, `Accepting`, `Archiving`, terminal `Rejected`, terminal `Merged`, terminal `Error`, `Stalled`, `Gated`, `Blocked`, `Queued`, `MergeWait`, or absent.

Manual/user resolve intent for an existing `MergeWait` row remains valid and may still transition that row to `ResolveWait` through the reducer-owned `ResolveMerge` command.

#### Scenario: archive completes while another change is resolving

**Given**: Change A is in active `Resolving` state
**And**: Change B has just been archived in parallel mode
**When**: the `ChangeArchived` event for B is processed
**Then**: B transitions to `ResolveWait`
**And**: B's derived display status is `resolve pending`
**And**: B is eligible for automatic retry after A's resolve completes

#### Scenario: archive completes while another change is rejecting

**Given**: Change A is in active `Rejecting` state
**And**: Change B has just been archived in parallel mode
**When**: the `ChangeArchived` event for B is processed
**Then**: B transitions to `ResolveWait`
**And**: B's derived display status is `resolve pending`
**And**: B is eligible for automatic retry after A's rejection review completes or fails

#### Scenario: archive completes while another change is applying

**Given**: Change A is in active `Applying` state
**And**: Change B has just been archived in parallel mode
**When**: the `ChangeArchived` event for B is processed
**Then**: B does not transition to `ResolveWait` because of A
**And**: B's derived display status is not `resolve pending` unless an explicit resolve intent is later recorded

#### Scenario: archive completes while another change is accepting

**Given**: Change A is in active `Accepting` state
**And**: Change B has just been archived in parallel mode
**When**: the `ChangeArchived` event for B is processed
**Then**: B does not transition to `ResolveWait` because of A
**And**: B's derived display status is not `resolve pending` unless an explicit resolve intent is later recorded

#### Scenario: terminal rejected change does not create resolve pending

**Given**: Change A is terminal `Rejected`
**And**: Change B has just been archived in parallel mode
**When**: the `ChangeArchived` event for B is processed
**Then**: B does not transition to `ResolveWait` because of A
**And**: B's derived display status is not `resolve pending` unless an explicit resolve intent is later recorded

#### Scenario: no active blocker starts immediate merge path

**Given**: no other change is actively `Resolving` or `Rejecting`
**And**: Change B has just been archived in parallel mode
**When**: post-archive dispatch is evaluated
**Then**: the orchestrator attempts the immediate merge/resolve path for B instead of recording automatic `ResolveWait`

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

The runtime state MUST distinguish at least the following blocker-adjacent concerns without collapsing them into a single `blocked` label:

- dependency wait reason (`blocked`, canonical concept: `dependency-blocked`)
- apply/rejecting resumable hold (`stalled`)
- acceptance gate observation (`gated`, canonical concept: `acceptance-gated`)

Derived display status exposed from reducer-owned runtime state SHALL preserve this distinction for consumers.

#### Scenario: dependency wait remains blocked
- **GIVEN** a change is queued for execution
- **AND** dependency analysis reports unresolved dependencies
- **WHEN** the reducer applies the dependency-blocked input
- **THEN** the wait reason becomes dependency `blocked`
- **AND** the derived display status is `blocked`
- **AND** the change is not labeled `stalled` or `gated`

#### Scenario: resumable apply hold is stalled
- **GIVEN** apply or rejecting review reports a resumable blocker for a change
- **WHEN** the reducer applies the hold transition
- **THEN** the lifecycle state becomes `stalled`
- **AND** terminal result remains `None`
- **AND** the derived display status is `stalled`
- **AND** the reducer does not reuse dependency `blocked` for this hold

#### Scenario: acceptance gate is exposed separately
- **GIVEN** acceptance reports a blocker observation before the next lifecycle step is chosen
- **WHEN** reducer-owned state or its derived event/status surface exposes that observation
- **THEN** the displayed wording is `gated`
- **AND** the canonical taxonomy identifies the observation as `acceptance-gated`
- **AND** the observation is distinguishable from dependency `blocked` and apply `stalled`

### Requirement: Resolve Wait Queue Ownership

The system SHALL treat `ResolveMerge` / `MergeWait` retry as reducer-owned scheduler intent, not as a TUI-local direct execution operation.

When the scheduler retries an archived Git merge and the merge path reaches a normal merge-ready state without unresolved conflicts, the runtime SHALL complete that merge through the normal merge/verification path and SHALL NOT start AI conflict resolution solely because the retry entered the resolve-capable code path.

Post-merge verification for this path SHALL accept repository-visible merge success without requiring the archived source branch tip to continue containing the pre-merge base after the merge commit has already integrated the change into the target branch.

Reducer-owned `ResolveWait` SHALL be considered schedulable work even when there are no queued active changes. Scheduler startup and idle/drained checks MUST include this reducer-owned work before deciding that a run has no work.

#### Scenario: reducer-owned resolve wait survives empty startup

**Given**: change `alpha` is stored in the shared reducer as `ResolveWait`
**And**: the scheduler starts with an empty active change list
**When**: the scheduler evaluates whether work is drained
**Then**: it treats `alpha` as pending scheduler-owned retry work
**And**: it does not emit only a zero-change completion without attempting the retry

#### Scenario: resolve wait is synchronized before drained exit

**Given**: shared reducer state contains one or more `ResolveWait` changes
**When**: the scheduler loop begins an iteration
**Then**: it synchronizes those IDs into executor retry state before checking whether queued, in-flight, resolve-wait, manual-resolve, and pending-merge work are all empty

### Requirement: Reducer-Owned Change Runtime State

The runtime state MUST distinguish at least the following blocker-adjacent concerns without collapsing them into a single `blocked` label:

- dependency wait reason (`blocked`, canonical concept: `dependency-blocked`)
- apply/rejecting resumable hold (`stalled`)
- acceptance gate observation (`gated`, canonical concept: `acceptance-gated`)

Derived display status exposed from reducer-owned runtime state SHALL preserve this distinction for consumers.

#### Scenario: dependency wait remains blocked
- **GIVEN** a change is queued for execution
- **AND** dependency analysis reports unresolved dependencies
- **WHEN** the reducer applies the dependency-blocked input
- **THEN** the wait reason becomes dependency `blocked`
- **AND** the derived display status is `blocked`
- **AND** the change is not labeled `stalled` or `gated`

#### Scenario: resumable apply hold is stalled
- **GIVEN** apply or rejecting review reports a resumable blocker for a change
- **WHEN** the reducer applies the hold transition
- **THEN** the lifecycle state becomes `stalled`
- **AND** terminal result remains `None`
- **AND** the derived display status is `stalled`
- **AND** the reducer does not reuse dependency `blocked` for this hold

#### Scenario: acceptance gate is exposed separately
- **GIVEN** acceptance reports a blocker observation before the next lifecycle step is chosen
- **WHEN** reducer-owned state or its derived event/status surface exposes that observation
- **THEN** the displayed wording is `gated`
- **AND** the canonical taxonomy identifies the observation as `acceptance-gated`
- **AND** the observation is distinguishable from dependency `blocked` and apply `stalled`

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

The runtime state MUST distinguish at least the following blocker-adjacent concerns without collapsing them into a single `blocked` label:

- dependency wait reason (`blocked`, canonical concept: `dependency-blocked`)
- apply/rejecting resumable hold (`stalled`)
- acceptance gate observation (`gated`, canonical concept: `acceptance-gated`)

Derived display status exposed from reducer-owned runtime state SHALL preserve this distinction for consumers.

#### Scenario: dependency wait remains blocked
- **GIVEN** a change is queued for execution
- **AND** dependency analysis reports unresolved dependencies
- **WHEN** the reducer applies the dependency-blocked input
- **THEN** the wait reason becomes dependency `blocked`
- **AND** the derived display status is `blocked`
- **AND** the change is not labeled `stalled` or `gated`

#### Scenario: resumable apply hold is stalled
- **GIVEN** apply or rejecting review reports a resumable blocker for a change
- **WHEN** the reducer applies the hold transition
- **THEN** the lifecycle state becomes `stalled`
- **AND** terminal result remains `None`
- **AND** the derived display status is `stalled`
- **AND** the reducer does not reuse dependency `blocked` for this hold

#### Scenario: acceptance gate is exposed separately
- **GIVEN** acceptance reports a blocker observation before the next lifecycle step is chosen
- **WHEN** reducer-owned state or its derived event/status surface exposes that observation
- **THEN** the displayed wording is `gated`
- **AND** the canonical taxonomy identifies the observation as `acceptance-gated`
- **AND** the observation is distinguishable from dependency `blocked` and apply `stalled`

### Requirement: Rejected terminal state remains distinct from errors

The terminal result MUST include `Rejected` as a permanent terminal state distinct from `Error`. A rejected change is one where rejecting review has confirmed the specification is unimplementable or otherwise out of scope for completion, requiring a rollback to the base branch with a documented reason.

#### Scenario: rejecting-confirmed change becomes rejected terminal state

- **GIVEN** a change is in `Rejecting`
- **AND** the rejection flow completes (`REJECTED.md` committed and worktree removed)
- **WHEN** the reducer applies the terminal rejection event
- **THEN** the terminal result becomes `Rejected`
- **AND** the derived display status is `rejected`

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

The runtime state MUST distinguish at least the following blocker-adjacent concerns without collapsing them into a single `blocked` label:

- dependency wait reason (`blocked`, canonical concept: `dependency-blocked`)
- apply/rejecting resumable hold (`stalled`)
- acceptance gate observation (`gated`, canonical concept: `acceptance-gated`)

Derived display status exposed from reducer-owned runtime state SHALL preserve this distinction for consumers.

#### Scenario: dependency wait remains blocked
- **GIVEN** a change is queued for execution
- **AND** dependency analysis reports unresolved dependencies
- **WHEN** the reducer applies the dependency-blocked input
- **THEN** the wait reason becomes dependency `blocked`
- **AND** the derived display status is `blocked`
- **AND** the change is not labeled `stalled` or `gated`

#### Scenario: resumable apply hold is stalled
- **GIVEN** apply or rejecting review reports a resumable blocker for a change
- **WHEN** the reducer applies the hold transition
- **THEN** the lifecycle state becomes `stalled`
- **AND** terminal result remains `None`
- **AND** the derived display status is `stalled`
- **AND** the reducer does not reuse dependency `blocked` for this hold

#### Scenario: acceptance gate is exposed separately
- **GIVEN** acceptance reports a blocker observation before the next lifecycle step is chosen
- **WHEN** reducer-owned state or its derived event/status surface exposes that observation
- **THEN** the displayed wording is `gated`
- **AND** the canonical taxonomy identifies the observation as `acceptance-gated`
- **AND** the observation is distinguishable from dependency `blocked` and apply `stalled`

### Requirement: Reducer Input Precedence and Idempotency

Workspace observations SHALL NOT regress a change from terminal `Merged` back to `MergeWait` when the change has already been integrated into the base branch, including fast-forward integration.

#### Scenario: Archived workspace observation does not regress fast-forward merged change

- **GIVEN** a change has already reached terminal `Merged`
- **AND** the integration happened via fast-forward rather than a merge commit
- **WHEN** a later `ChangesRefreshed` event observes the workspace as archived
- **THEN** the reducer keeps the terminal state as `Merged`
- **AND** the derived display status does not regress to `merge wait`

### Requirement: Reducer-Owned Change Runtime State

The runtime state MUST distinguish at least the following blocker-adjacent concerns without collapsing them into a single `blocked` label:

- dependency wait reason (`blocked`, canonical concept: `dependency-blocked`)
- apply/rejecting resumable hold (`stalled`)
- acceptance gate observation (`gated`, canonical concept: `acceptance-gated`)

Derived display status exposed from reducer-owned runtime state SHALL preserve this distinction for consumers.

#### Scenario: dependency wait remains blocked
- **GIVEN** a change is queued for execution
- **AND** dependency analysis reports unresolved dependencies
- **WHEN** the reducer applies the dependency-blocked input
- **THEN** the wait reason becomes dependency `blocked`
- **AND** the derived display status is `blocked`
- **AND** the change is not labeled `stalled` or `gated`

#### Scenario: resumable apply hold is stalled
- **GIVEN** apply or rejecting review reports a resumable blocker for a change
- **WHEN** the reducer applies the hold transition
- **THEN** the lifecycle state becomes `stalled`
- **AND** terminal result remains `None`
- **AND** the derived display status is `stalled`
- **AND** the reducer does not reuse dependency `blocked` for this hold

#### Scenario: acceptance gate is exposed separately
- **GIVEN** acceptance reports a blocker observation before the next lifecycle step is chosen
- **WHEN** reducer-owned state or its derived event/status surface exposes that observation
- **THEN** the displayed wording is `gated`
- **AND** the canonical taxonomy identifies the observation as `acceptance-gated`
- **AND** the observation is distinguishable from dependency `blocked` and apply `stalled`

### Requirement: WebSocket change status consistency with TUI

Server-mode WebSocket API SHALL produce the same set of display status strings as `ChangeRuntimeState.display_status()`. The system MUST NOT maintain a separate mapping from workspace states to display strings that diverges from the reducer-derived status vocabulary.

#### Scenario: All display statuses are representable in WebSocket payloads

- **GIVEN** the reducer can produce any of: `not queued`, `queued`, `blocked`, `applying`, `accepting`, `rejecting`, `archiving`, `resolving`, `merge wait`, `resolve pending`, `archived`, `merged`, `rejected`, `error`, `stopped`
- **WHEN** a WebSocket client receives a change list
- **THEN** the status field for each change is one of the above values

### Requirement: Scheduler dispatch derives queued candidates from reducer state

The parallel scheduler's decision to dispatch queued changes SHALL be derived from reducer-observable state (queue intent, active execution stage, available slots) rather than transient event flags. This ensures that changes with `QueueIntent::Queued` in the reducer are always considered for dispatch when execution capacity exists.

#### Scenario: Reducer queued change is visible to scheduler dispatch

- **GIVEN** a change has `QueueIntent::Queued` in the reducer
- **AND** no activity stage is active for that change
- **AND** available execution slots are greater than zero
- **WHEN** the scheduler evaluates dispatch candidates
- **THEN** the change is included in the re-analysis candidate set
- **AND** the scheduler does not require a separate event flag to consider this change

### Requirement: post-archive-merge-dispatch

When a change is archived in parallel mode, the orchestrator must attempt to merge immediately unless another non-terminal change is actively occupying the automatic retry blocker lane. The only lifecycle activities that occupy that lane are `Resolving` and `Rejecting` on another change.

Automatic `ResolveWait` / `resolve pending` MUST NOT be created solely because another change is `Applying`, `Accepting`, `Archiving`, terminal `Rejected`, terminal `Merged`, terminal `Error`, `Stalled`, `Gated`, `Blocked`, `Queued`, `MergeWait`, or absent.

Manual/user resolve intent for an existing `MergeWait` row remains valid and may still transition that row to `ResolveWait` through the reducer-owned `ResolveMerge` command.

#### Scenario: archive completes while another change is resolving

**Given**: Change A is in active `Resolving` state
**And**: Change B has just been archived in parallel mode
**When**: the `ChangeArchived` event for B is processed
**Then**: B transitions to `ResolveWait`
**And**: B's derived display status is `resolve pending`
**And**: B is eligible for automatic retry after A's resolve completes

#### Scenario: archive completes while another change is rejecting

**Given**: Change A is in active `Rejecting` state
**And**: Change B has just been archived in parallel mode
**When**: the `ChangeArchived` event for B is processed
**Then**: B transitions to `ResolveWait`
**And**: B's derived display status is `resolve pending`
**And**: B is eligible for automatic retry after A's rejection review completes or fails

#### Scenario: archive completes while another change is applying

**Given**: Change A is in active `Applying` state
**And**: Change B has just been archived in parallel mode
**When**: the `ChangeArchived` event for B is processed
**Then**: B does not transition to `ResolveWait` because of A
**And**: B's derived display status is not `resolve pending` unless an explicit resolve intent is later recorded

#### Scenario: archive completes while another change is accepting

**Given**: Change A is in active `Accepting` state
**And**: Change B has just been archived in parallel mode
**When**: the `ChangeArchived` event for B is processed
**Then**: B does not transition to `ResolveWait` because of A
**And**: B's derived display status is not `resolve pending` unless an explicit resolve intent is later recorded

#### Scenario: terminal rejected change does not create resolve pending

**Given**: Change A is terminal `Rejected`
**And**: Change B has just been archived in parallel mode
**When**: the `ChangeArchived` event for B is processed
**Then**: B does not transition to `ResolveWait` because of A
**And**: B's derived display status is not `resolve pending` unless an explicit resolve intent is later recorded

#### Scenario: no active blocker starts immediate merge path

**Given**: no other change is actively `Resolving` or `Rejecting`
**And**: Change B has just been archived in parallel mode
**When**: post-archive dispatch is evaluated
**Then**: the orchestrator attempts the immediate merge/resolve path for B instead of recording automatic `ResolveWait`

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

When the scheduler retries an archived Git merge and the merge path reaches a normal merge-ready state without unresolved conflicts, the runtime SHALL complete that merge through the normal merge/verification path and SHALL NOT start AI conflict resolution solely because the retry entered the resolve-capable code path.

Post-merge verification for this path SHALL accept repository-visible merge success without requiring the archived source branch tip to continue containing the pre-merge base after the merge commit has already integrated the change into the target branch.

Reducer-owned `ResolveWait` SHALL be considered schedulable work even when there are no queued active changes. Scheduler startup and idle/drained checks MUST include this reducer-owned work before deciding that a run has no work.

#### Scenario: reducer-owned resolve wait survives empty startup

**Given**: change `alpha` is stored in the shared reducer as `ResolveWait`
**And**: the scheduler starts with an empty active change list
**When**: the scheduler evaluates whether work is drained
**Then**: it treats `alpha` as pending scheduler-owned retry work
**And**: it does not emit only a zero-change completion without attempting the retry

#### Scenario: resolve wait is synchronized before drained exit

**Given**: shared reducer state contains one or more `ResolveWait` changes
**When**: the scheduler loop begins an iteration
**Then**: it synchronizes those IDs into executor retry state before checking whether queued, in-flight, resolve-wait, manual-resolve, and pending-merge work are all empty
