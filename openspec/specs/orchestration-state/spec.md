## Purpose

Provide a single, reducer-owned model for tracking the runtime lifecycle of each change across serial and parallel execution modes. All display status is derived from this shared state; consumers never own an independent lifecycle copy.

## Requirements

### Requirement: Reducer-Owned Change Runtime State

The system SHALL maintain reducer-owned runtime state for each change in `OrchestratorState`.

The runtime state MUST distinguish at least the following concerns:

- queue intent
- active execution stage
- wait reason
- terminal result
- workspace observation summary
- execution mode (Serial or Parallel)

Display status exposed to consumers MAY be derived from this runtime state, but consumers SHALL NOT own an independent lifecycle copy.

#### Scenario: Runtime state preserves queued intent while blocked

- **GIVEN** a change is queued for execution
- **AND** dependency analysis reports unresolved dependencies
- **WHEN** the reducer applies the dependency-blocked input
- **THEN** the runtime state records queued intent
- **AND** the wait reason becomes blocked with dependency details
- **AND** the derived display status is `blocked`

#### Scenario: Runtime state distinguishes merge wait from archived result

- **GIVEN** archive has completed for a change in parallel execution mode
- **WHEN** the reducer applies the `ChangeArchived` event
- **THEN** the wait reason becomes merge-wait
- **AND** the terminal state remains `None` (not yet terminal)
- **AND** the derived display status is `merge wait`

### Requirement: Reducer Input Precedence and Idempotency

The reducer SHALL accept mutations only through structured inputs: user commands, execution events, and workspace observations.

The reducer MUST be idempotent for duplicate inputs and MUST ignore stale inputs that would regress terminal state.

Execution events SHALL own active-stage and terminal transitions. Workspace observations SHALL reconcile durable wait/recovery state and MUST NOT override an active execution stage.

#### Scenario: Duplicate event is a no-op

- **GIVEN** a change is already in an applying activity state
- **WHEN** the same `ApplyStarted` event is processed again
- **THEN** the reducer leaves the runtime state unchanged
- **AND** no invalid regression occurs

#### Scenario: Late failure does not regress merged state

- **GIVEN** a change is already in terminal merged state
- **WHEN** a stale `ResolveFailed` or `ApplyFailed` event arrives
- **THEN** the reducer ignores the stale event
- **AND** the runtime state remains merged

#### Scenario: Observation does not override active resolve

- **GIVEN** a change is currently resolving
- **WHEN** auto-refresh observes that the worktree is archived and ahead of base
- **THEN** the reducer stores the observation
- **AND** the displayed status remains `resolving`

### Requirement: Resolve Wait Queue Ownership

The system SHALL own the resolve wait queue in shared orchestration state rather than in TUI-local lifecycle state.

`ResolveWait` SHALL represent reducer-owned queued resolve intent while another resolve is active.

Manual resolve lifecycle events that clear or complete queued resolve intent MUST also be applied to the shared orchestration reducer before later refresh-driven display reconciliation can occur.

#### Scenario: Resolve wait queue is reducer-owned

- **GIVEN** one change is currently resolving
- **AND** the user requests resolve for another change in `MergeWait`
- **WHEN** the reducer processes the command
- **THEN** the second change enters `ResolveWait`
- **AND** the change_id is stored in the shared resolve wait queue

#### Scenario: ResolveWait is not reconstructed from workspace only

- **GIVEN** a change has an archived workspace that is still ahead of base
- **WHEN** the system rebuilds state from workspace observation alone
- **THEN** the reducer may recover `MergeWait`
- **AND** the reducer does not recover `ResolveWait` unless the shared resolve wait queue contains that change

#### Scenario: Manual resolve completion clears reducer-owned resolve wait

- **GIVEN** the user has triggered manual resolve for a change that entered `ResolveWait`
- **AND** the shared reducer currently derives display status `resolve pending`
- **WHEN** the manual resolve completes successfully and the merge result becomes terminal
- **THEN** the shared reducer clears the queued resolve wait for that change
- **AND** subsequent `ChangesRefreshed` reconciliation does not derive `resolve pending` for the merged change

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

The system SHALL own the resolve wait queue in shared orchestration state rather than in TUI-local lifecycle state.

`ResolveWait` SHALL represent reducer-owned queued resolve intent while another resolve is active, or a deferred change that has been auto-promoted into the next resolve flow after dependency or merge preconditions are satisfied.

Manual resolve lifecycle events that clear or complete queued resolve intent MUST also be applied to the shared orchestration reducer before later refresh-driven display reconciliation can occur.

Workspace observation alone MAY recover `MergeWait` for archived-but-unmerged workspaces, but it MUST NOT erase reducer-owned auto-resolve intent that was established from `MergeDeferred` reason tracking.

#### Scenario: Auto-promoted deferred change enters reducer-owned resolve wait
- **GIVEN** a change was deferred because another merge or resolve had to complete first
- **WHEN** that prerequisite completes and the reducer receives the promotion signal
- **THEN** the change enters reducer-owned `ResolveWait` or `Resolving`
- **AND** subsequent refresh reconciliation does not regress it to `MergeWait`

#### Scenario: Workspace refresh does not overwrite auto-resolve intent
- **GIVEN** a change has already been auto-promoted from deferred merge waiting into reducer-owned resolve intent
- **WHEN** a later `ChangesRefreshed` event observes the workspace as archived
- **THEN** the reducer preserves the auto-resolve wait state
- **AND** the displayed status does not regress to a stale manual-wait state

## Requirements

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

When a change is archived in parallel mode, the orchestrator must attempt to merge or queue the change for resolve, rather than leaving it in MergeWait indefinitely.

#### Scenario: archive-completes-while-resolve-active

**Given**: Change A is in Resolving state and change B has just been archived in parallel mode
**When**: The ChangeArchived event for B is processed by the TUI orchestrator
**Then**: B transitions to ResolveWait (not MergeWait) and is added to the resolve queue for automatic execution after A's resolve completes

#### Scenario: archive-completes-no-active-resolve

**Given**: No resolve is currently active and change B has just been archived in parallel mode
**When**: The ChangeArchived event for B is processed by the TUI orchestrator
**Then**: An immediate merge attempt is initiated for B (via ResolveMerge command)


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
