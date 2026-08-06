## Purpose

Provide a single, reducer-owned model for tracking the runtime lifecycle of each change across serial and parallel execution modes. All display status is derived from this shared state; consumers never own an independent lifecycle copy.

## Requirements

### Requirement: Reducer-Owned Change Runtime State

Reducer-owned orchestration state SHALL reflect the latest repository-visible terminal outcome for a change. A recoverable error terminal state SHALL NOT remain the current display state after the same change later emits archive, merge, push, or resolve success events.

Success events MAY supersede `TerminalState::Error` for the same change because errors from acceptance, apply, archive, resolve, or push attempts are recoverable until the change reaches the terminal success required by its invocation mode or final rejection. Success events MUST NOT overwrite final rejection state.

Without opt-in upstream integration, successful cumulative base integration SHALL transition a parallel change to terminal `Merged`. With opt-in upstream integration, local cumulative base integration SHALL remain non-terminal publication progress, and only change-scoped `PushCompleted` emitted after selected-remote observation confirms cumulative HEAD reachability SHALL transition the change to terminal `Pushed`. An opted-in change MUST NOT be displayed as final `merged` while publication remains pending, failed, stalled, or unconfirmed.

A recoverable error terminal state MUST gate ordinary apply dispatch. Explicit retry MUST be limited to recoverable work and MUST NOT requeue final rejected, merged, pushed, or archived terminal states. Retry of an opted-in locally integrated but unpublished change MUST resume upstream publication and MUST NOT create ordinary apply or acceptance dispatch. In persistent local TUI, a publication failure or stall that exhausts its bounded automatic cycle MUST project into the existing operator-visible recoverable error flow, where F5 or the equivalent local web-control retry starts explicit publication retry. The base lane MUST remain closed to later completed-result integration until remote confirmation succeeds or the operator stops orchestration.

#### Scenario: disabled cumulative merge becomes merged terminal

**Given**: change `alpha` completes cumulative base integration without upstream integration enabled
**When**: the reducer receives merge success
**Then**: terminal state becomes `Merged`
**And**: display status is `merged`

#### Scenario: opted-in local merge remains non-terminal

**Given**: change `alpha` is running with upstream integration enabled
**When**: its archived result merges successfully into cumulative base
**Then**: reducer-owned state records publication progress without terminal `Merged`
**And**: the display does not claim final `merged` success
**And**: ordinary apply and acceptance dispatch for `alpha` remain disabled

#### Scenario: remote-confirmed publication becomes pushed terminal

**Given**: change `alpha` is locally integrated with upstream integration enabled
**And**: Conflux confirms through remote observation that cumulative HEAD is reachable from the selected remote base
**When**: `alpha` receives change-scoped `PushCompleted`
**Then**: terminal state becomes `Pushed`
**And**: display status is `pushed`
**And**: `alpha` is not displayed as `merged`

#### Scenario: publication failure remains resumable

**Given**: change `alpha` is locally integrated with upstream integration enabled
**When**: verification, push, or remote confirmation fails
**Then**: `alpha` does not become `Merged` or `Pushed`
**And**: reducer-owned state exposes recoverable publication failure or wait evidence
**And**: explicit retry returns `alpha` to publication work rather than ordinary apply work

#### Scenario: persistent TUI retries failed publication

**Given**: local TUI owns publication for change `alpha`
**And**: the bounded publication cycle has ended in a recoverable failure or stall
**When**: the TUI projects its reducer-owned state
**Then**: it displays `alpha` through the existing recoverable Error-mode interaction
**And**: F5 or the equivalent local web-control action is available as explicit retry
**And**: later completed results remain waiting before base integration
**And**: no ordinary apply or acceptance dispatch for `alpha` is created

#### Scenario: successful TUI retry releases waiting base integration

**Given**: local TUI displays recoverable publication failure for change `alpha`
**And**: change `beta` is waiting before cumulative-base integration
**When**: the operator explicitly retries and Conflux remotely confirms `alpha`
**Then**: `alpha` becomes terminal `Pushed`
**And**: the base lane becomes available for `beta`
**And**: the TUI remains active

#### Scenario: late publication success supersedes recoverable failure

**Given**: change `alpha` has recoverable publication error state
**And**: already-running or retried repository work later confirms cumulative HEAD on the selected remote
**When**: `PushCompleted(alpha)` arrives
**Then**: terminal state becomes `Pushed`
**And**: no ordinary apply dispatch is created

#### Scenario: pushed terminal is not retryable

**Given**: change `alpha` has terminal state `Pushed`
**When**: an explicit retry transition is requested
**Then**: `alpha` remains `Pushed`
**And**: it is not reintroduced as apply, acceptance, merge, or publication work

### Requirement: Reducer Input Precedence and Idempotency

Workspace observations and refresh-derived archive-complete evidence SHALL NOT regress reducer-owned active, pending, or terminal lifecycle states to `MergeWait` without concrete manual deferral evidence.

A `ChangesRefreshed` event containing a change in `merge_wait_ids` represents archived-but-not-yet-merged workspace evidence. That evidence MAY preserve or restore an already-established manual `MergeWait`, but it MUST NOT override `ActivityState::Resolving`, `WaitState::ResolveWait`, `WaitState::RejectWait`, `ActivityState::Rejecting`, or terminal states.

<!-- Expected canonical result after archive: `orchestration-state` will treat refresh-derived merge-wait evidence as lower precedence than reducer-owned active/pending/terminal state. -->

#### Scenario: refresh evidence does not regress resolving

**Given**: change `alpha` has `ActivityState::Resolving`
**When**: a `ChangesRefreshed` event includes `alpha` in `merge_wait_ids`
**Then**: `alpha` remains `resolving`
**And**: `alpha` is not changed to `merge wait`

#### Scenario: refresh evidence does not regress resolve pending

**Given**: change `alpha` has `WaitState::ResolveWait`
**When**: a `ChangesRefreshed` event includes `alpha` in `merge_wait_ids`
**Then**: `alpha` remains `resolve pending`
**And**: reducer-owned scheduler retry membership remains available

#### Scenario: refresh evidence can preserve concrete manual merge wait

**Given**: change `alpha` has already received concrete manual deferral evidence and is in `WaitState::MergeWait`
**When**: a `ChangesRefreshed` event includes `alpha` in `merge_wait_ids`
**Then**: `alpha` remains `merge wait`
**And**: no normal queue intent is reintroduced for `alpha`

### Requirement: Resolve Wait Queue Ownership

The system SHALL treat `ResolveMerge` / `MergeWait` retry as reducer-owned scheduler intent, not as a TUI-local direct execution operation.

When a merge attempt is deferred because it is auto-resumable, such as another merge or resolve lane currently occupying merge capacity, the reducer SHALL represent the change as `ResolveWait` and keep it eligible for scheduler-owned retry.

When a merge attempt is deferred because manual user action is required, such as a dirty base working tree with uncommitted changes, the reducer SHALL represent the change as `MergeWait` and SHALL remove normal queue intent for that change. Manual merge deferral MUST NOT cause scheduler queue reconciliation to re-dispatch the archived workspace as ordinary queued work.

An explicit `ResolveMerge` command remains the way to retry a manual merge-wait change after the user resolves the manual blocker. If repository-visible evidence still shows an archived-but-not-merged change is waiting for manual merge retry, the reducer MUST accept retry intent in a form that remains scheduler-consumable and MUST NOT silently drop it while the TUI continues to display pending retry.

After a manual `MergeDeferred(auto_resumable=false)` returns a change to `MergeWait`, a later explicit `ResolveMerge` for the same change MUST be treated as a fresh user retry intent. Executor-local retry dedupe, dirty-state tracking, or previous dispatch snapshots MUST NOT suppress this retry after the manual blocker has been resolved.

If an explicit `ResolveMerge` for a `MergeWait` change is accepted after a previous dirty manual deferral, the active or newly-started scheduler MUST consume the same shared reducer-owned `ResolveWait` membership even when there are no ordinary queued apply candidates. A scheduler run started for this purpose MUST NOT replace caller-owned reducer state with a fresh reducer and MUST NOT complete as an ordinary zero-change run while accepted retry membership remains.

After a change has reached repository-visible base integration, later stale duplicate merge outcomes for the same change MUST NOT regress the reducer-visible lifecycle from `Merged` to `MergeWait` or `ResolveWait`.

The shared reducer state that accepts `ResolveMerge` MUST be the same authoritative reducer state observed by the scheduler/executor that consumes the retry. A service or executor construction path MUST NOT replace caller-owned reducer state with a fresh empty reducer state after retry intent has been accepted. State synchronization may copy reducer-owned lane-wait membership into executor-local caches, but the copied cache MUST NOT become an independent source of truth that can make the UI show `resolve pending` after reducer-owned membership has been cleared.

<!-- Expected canonical result after archive: `orchestration-state` will require explicit manual retry after dirty manual deferral to be consumed by the same shared scheduler/reducer path even when normal queue work is empty. -->

#### Scenario: dirty base manual retry returns to merge wait

**Given**: change `alpha` is archive-complete and ready for post-archive merge handling
**And**: the base repository has uncommitted changes
**When**: the user requests merge retry for `alpha`
**Then**: the merge attempt is deferred with `MergeDeferred(alpha, auto_resumable=false)`
**And**: reducer-visible status for `alpha` becomes `merge wait`
**And**: `alpha` is removed from reducer-owned resolve-wait membership
**And**: `alpha` is not reintroduced as ordinary queued apply work

#### Scenario: explicit retry after base clean starts scheduler-consumed retry

**Given**: change `alpha` is in reducer-visible `merge wait` due to dirty-base manual deferral
**And**: `alpha` still has an archive-complete workspace that is not merged to the base branch
**And**: the base repository has become clean
**When**: the user requests merge retry for `alpha` again
**Then**: `ReducerCommand::ResolveMerge(alpha)` is accepted
**And**: the reducer records `alpha` in `ResolveWait`
**And**: the active or newly-started scheduler consumes that same reducer-owned `ResolveWait` intent
**And**: retry evaluation reaches the merge attempt path for `alpha`
**And**: if no blocker remains, `alpha` can transition to `merged`

#### Scenario: clean retry is not suppressed by prior dirty dispatch

**Given**: change `alpha` previously had a manual retry dispatched
**And**: that retry returned to `MergeWait` through dirty workspace/base manual deferral
**And**: executor-local retry dispatch snapshots or dirty-state observations still remember the previous attempt
**And**: the base/workspace retry preconditions are now clean
**When**: the user requests merge retry for `alpha` again
**Then**: stale executor-local state does not suppress retry dispatch
**And**: retry evaluation uses current workspace and base repository state
**And**: retry evaluation reaches the merge attempt path for `alpha`

#### Scenario: zero-normal-queue retry consumes shared reducer membership

**Given**: no ordinary queued apply candidates exist
**And**: no scheduler is currently running
**And**: the shared reducer accepts `ResolveMerge(alpha)` for a manual `merge wait` row
**When**: the TUI command handler starts a scheduler run for the manual retry
**Then**: the scheduler observes the caller-owned shared reducer state containing `ResolveWait(alpha)`
**And**: the scheduler evaluates base-lane waiters before reporting ordinary zero-change completion
**And**: `alpha` leaves `ResolveWait` only through scheduler-owned events or visible blocker evidence

#### Scenario: live scheduler notification consumes lane waiter without ordinary queue work

**Given**: the TUI logs `Scheduled merge-wait retry intent for 'alpha'; notified existing scheduler`
**And**: the scheduler is already running
**And**: there are no ordinary queued apply candidates remaining
**When**: reducer-owned `ResolveWait(alpha)` exists
**Then**: the scheduler wakes and evaluates base-lane waiters
**And**: the retry does not require another queued change or another user keypress to make progress

#### Scenario: explicit manual retry is not suppressed by stale dispatch dedupe

**Given**: change `alpha` previously had a `ResolveWait` retry dispatched
**And**: that retry returned to manual `merge wait` through `MergeDeferred(alpha, auto_resumable=false)`
**When**: the user resolves the manual blocker and requests merge retry for `alpha` again
**Then**: stale executor-local dispatch snapshots or dirty-state observations do not suppress retry dispatch
**And**: the retry is evaluated against current workspace and base repository state

#### Scenario: accepted retry emits evidence when still blocked

**Given**: explicit retry for `alpha` is accepted into reducer-owned `ResolveWait`
**And**: retry evaluation still cannot start or complete merge handling
**When**: the scheduler evaluates the retry
**Then**: the system emits log or event evidence identifying the remaining blocker
**And**: `alpha` remains in the correct reducer-visible wait state for that blocker

### Requirement: Execution Mode Determines Archive Terminal Semantics

`ChangeArchived` SHALL NOT be terminal by itself. Archive completion SHALL enter post-archive handling according to reducer-owned base-mutating lane state and the configured merge or push action:

- when another non-terminal change occupies the base-mutating lane with `Resolving` or `Rejecting`, the archived change SHALL become `ResolveWait` and remain scheduler-consumable;
- when no base-mutating lane blocker exists and no concrete manual blocker has been observed, the archived change SHALL become active `Resolving`;
- only concrete manual deferral evidence SHALL set `MergeWait`.

#### Scenario: archive without blocker enters resolving

**Given**: no other non-terminal change is `Resolving` or `Rejecting`
**When**: change `alpha` receives a `ChangeArchived` event
**Then**: `alpha` has `ActivityState::Resolving`
**And**: `alpha` is not terminal solely because archive completed

#### Scenario: archive waits behind active base-mutating lane

**Given**: change `beta` is non-terminal and actively `Resolving` or `Rejecting`
**When**: change `alpha` receives a `ChangeArchived` event
**Then**: `alpha` has `WaitState::ResolveWait`
**And**: `alpha` remains scheduler-consumable

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

An explicit `ResolveMerge` command remains the way to retry a manual merge-wait change after the user resolves the manual blocker. If repository-visible evidence still shows an archived-but-not-merged change is waiting for manual merge retry, the reducer MUST accept retry intent in a form that remains scheduler-consumable and MUST NOT silently drop it while the TUI continues to display pending retry.

After a manual `MergeDeferred(auto_resumable=false)` returns a change to `MergeWait`, a later explicit `ResolveMerge` for the same change MUST be treated as a fresh user retry intent. Executor-local retry dedupe, dirty-state tracking, or previous dispatch snapshots MUST NOT suppress this retry after the manual blocker has been resolved.

If an explicit `ResolveMerge` for a `MergeWait` change is accepted after a previous dirty manual deferral, the active or newly-started scheduler MUST consume the same shared reducer-owned `ResolveWait` membership even when there are no ordinary queued apply candidates. A scheduler run started for this purpose MUST NOT replace caller-owned reducer state with a fresh reducer and MUST NOT complete as an ordinary zero-change run while accepted retry membership remains.

After a change has reached repository-visible base integration, later stale duplicate merge outcomes for the same change MUST NOT regress the reducer-visible lifecycle from `Merged` to `MergeWait` or `ResolveWait`.

The shared reducer state that accepts `ResolveMerge` MUST be the same authoritative reducer state observed by the scheduler/executor that consumes the retry. A service or executor construction path MUST NOT replace caller-owned reducer state with a fresh empty reducer state after retry intent has been accepted. State synchronization may copy reducer-owned lane-wait membership into executor-local caches, but the copied cache MUST NOT become an independent source of truth that can make the UI show `resolve pending` after reducer-owned membership has been cleared.

<!-- Expected canonical result after archive: `orchestration-state` will require explicit manual retry after dirty manual deferral to be consumed by the same shared scheduler/reducer path even when normal queue work is empty. -->

#### Scenario: dirty base manual retry returns to merge wait

**Given**: change `alpha` is archive-complete and ready for post-archive merge handling
**And**: the base repository has uncommitted changes
**When**: the user requests merge retry for `alpha`
**Then**: the merge attempt is deferred with `MergeDeferred(alpha, auto_resumable=false)`
**And**: reducer-visible status for `alpha` becomes `merge wait`
**And**: `alpha` is removed from reducer-owned resolve-wait membership
**And**: `alpha` is not reintroduced as ordinary queued apply work

#### Scenario: explicit retry after base clean starts scheduler-consumed retry

**Given**: change `alpha` is in reducer-visible `merge wait` due to dirty-base manual deferral
**And**: `alpha` still has an archive-complete workspace that is not merged to the base branch
**And**: the base repository has become clean
**When**: the user requests merge retry for `alpha` again
**Then**: `ReducerCommand::ResolveMerge(alpha)` is accepted
**And**: the reducer records `alpha` in `ResolveWait`
**And**: the active or newly-started scheduler consumes that same reducer-owned `ResolveWait` intent
**And**: retry evaluation reaches the merge attempt path for `alpha`
**And**: if no blocker remains, `alpha` can transition to `merged`

#### Scenario: clean retry is not suppressed by prior dirty dispatch

**Given**: change `alpha` previously had a manual retry dispatched
**And**: that retry returned to `MergeWait` through dirty workspace/base manual deferral
**And**: executor-local retry dispatch snapshots or dirty-state observations still remember the previous attempt
**And**: the base/workspace retry preconditions are now clean
**When**: the user requests merge retry for `alpha` again
**Then**: stale executor-local state does not suppress retry dispatch
**And**: retry evaluation uses current workspace and base repository state
**And**: retry evaluation reaches the merge attempt path for `alpha`

#### Scenario: zero-normal-queue retry consumes shared reducer membership

**Given**: no ordinary queued apply candidates exist
**And**: no scheduler is currently running
**And**: the shared reducer accepts `ResolveMerge(alpha)` for a manual `merge wait` row
**When**: the TUI command handler starts a scheduler run for the manual retry
**Then**: the scheduler observes the caller-owned shared reducer state containing `ResolveWait(alpha)`
**And**: the scheduler evaluates base-lane waiters before reporting ordinary zero-change completion
**And**: `alpha` leaves `ResolveWait` only through scheduler-owned events or visible blocker evidence

#### Scenario: live scheduler notification consumes lane waiter without ordinary queue work

**Given**: the TUI logs `Scheduled merge-wait retry intent for 'alpha'; notified existing scheduler`
**And**: the scheduler is already running
**And**: there are no ordinary queued apply candidates remaining
**When**: reducer-owned `ResolveWait(alpha)` exists
**Then**: the scheduler wakes and evaluates base-lane waiters
**And**: the retry does not require another queued change or another user keypress to make progress

#### Scenario: explicit manual retry is not suppressed by stale dispatch dedupe

**Given**: change `alpha` previously had a `ResolveWait` retry dispatched
**And**: that retry returned to manual `merge wait` through `MergeDeferred(alpha, auto_resumable=false)`
**When**: the user resolves the manual blocker and requests merge retry for `alpha` again
**Then**: stale executor-local dispatch snapshots or dirty-state observations do not suppress retry dispatch
**And**: the retry is evaluated against current workspace and base repository state

#### Scenario: accepted retry emits evidence when still blocked

**Given**: explicit retry for `alpha` is accepted into reducer-owned `ResolveWait`
**And**: retry evaluation still cannot start or complete merge handling
**When**: the scheduler evaluates the retry
**Then**: the system emits log or event evidence identifying the remaining blocker
**And**: `alpha` remains in the correct reducer-visible wait state for that blocker

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

Reducer-owned orchestration state SHALL reflect the latest repository-visible terminal outcome for a change. A recoverable error terminal state SHALL NOT remain the current display state after the same change later emits archive, merge, push, or resolve success events.

Success events MAY supersede `TerminalState::Error` for the same change because errors from acceptance, apply, archive, resolve, or push attempts are recoverable until the change reaches the terminal success required by its invocation mode or final rejection. Success events MUST NOT overwrite final rejection state.

Without opt-in upstream integration, successful cumulative base integration SHALL transition a parallel change to terminal `Merged`. With opt-in upstream integration, local cumulative base integration SHALL remain non-terminal publication progress, and only change-scoped `PushCompleted` emitted after selected-remote observation confirms cumulative HEAD reachability SHALL transition the change to terminal `Pushed`. An opted-in change MUST NOT be displayed as final `merged` while publication remains pending, failed, stalled, or unconfirmed.

A recoverable error terminal state MUST gate ordinary apply dispatch. Explicit retry MUST be limited to recoverable work and MUST NOT requeue final rejected, merged, pushed, or archived terminal states. Retry of an opted-in locally integrated but unpublished change MUST resume upstream publication and MUST NOT create ordinary apply or acceptance dispatch. In persistent local TUI, a publication failure or stall that exhausts its bounded automatic cycle MUST project into the existing operator-visible recoverable error flow, where F5 or the equivalent local web-control retry starts explicit publication retry. The base lane MUST remain closed to later completed-result integration until remote confirmation succeeds or the operator stops orchestration.

#### Scenario: disabled cumulative merge becomes merged terminal

**Given**: change `alpha` completes cumulative base integration without upstream integration enabled
**When**: the reducer receives merge success
**Then**: terminal state becomes `Merged`
**And**: display status is `merged`

#### Scenario: opted-in local merge remains non-terminal

**Given**: change `alpha` is running with upstream integration enabled
**When**: its archived result merges successfully into cumulative base
**Then**: reducer-owned state records publication progress without terminal `Merged`
**And**: the display does not claim final `merged` success
**And**: ordinary apply and acceptance dispatch for `alpha` remain disabled

#### Scenario: remote-confirmed publication becomes pushed terminal

**Given**: change `alpha` is locally integrated with upstream integration enabled
**And**: Conflux confirms through remote observation that cumulative HEAD is reachable from the selected remote base
**When**: `alpha` receives change-scoped `PushCompleted`
**Then**: terminal state becomes `Pushed`
**And**: display status is `pushed`
**And**: `alpha` is not displayed as `merged`

#### Scenario: publication failure remains resumable

**Given**: change `alpha` is locally integrated with upstream integration enabled
**When**: verification, push, or remote confirmation fails
**Then**: `alpha` does not become `Merged` or `Pushed`
**And**: reducer-owned state exposes recoverable publication failure or wait evidence
**And**: explicit retry returns `alpha` to publication work rather than ordinary apply work

#### Scenario: persistent TUI retries failed publication

**Given**: local TUI owns publication for change `alpha`
**And**: the bounded publication cycle has ended in a recoverable failure or stall
**When**: the TUI projects its reducer-owned state
**Then**: it displays `alpha` through the existing recoverable Error-mode interaction
**And**: F5 or the equivalent local web-control action is available as explicit retry
**And**: later completed results remain waiting before base integration
**And**: no ordinary apply or acceptance dispatch for `alpha` is created

#### Scenario: successful TUI retry releases waiting base integration

**Given**: local TUI displays recoverable publication failure for change `alpha`
**And**: change `beta` is waiting before cumulative-base integration
**When**: the operator explicitly retries and Conflux remotely confirms `alpha`
**Then**: `alpha` becomes terminal `Pushed`
**And**: the base lane becomes available for `beta`
**And**: the TUI remains active

#### Scenario: late publication success supersedes recoverable failure

**Given**: change `alpha` has recoverable publication error state
**And**: already-running or retried repository work later confirms cumulative HEAD on the selected remote
**When**: `PushCompleted(alpha)` arrives
**Then**: terminal state becomes `Pushed`
**And**: no ordinary apply dispatch is created

#### Scenario: pushed terminal is not retryable

**Given**: change `alpha` has terminal state `Pushed`
**When**: an explicit retry transition is requested
**Then**: `alpha` remains `Pushed`
**And**: it is not reintroduced as apply, acceptance, merge, or publication work

### Requirement: Resolve Wait Queue Ownership

The system SHALL treat `ResolveMerge` / `MergeWait` retry as reducer-owned scheduler intent, not as a TUI-local direct execution operation.

When a merge attempt is deferred because it is auto-resumable, such as another merge or resolve lane currently occupying merge capacity, the reducer SHALL represent the change as `ResolveWait` and keep it eligible for scheduler-owned retry.

When a merge attempt is deferred because manual user action is required, such as a dirty base working tree with uncommitted changes, the reducer SHALL represent the change as `MergeWait` and SHALL remove normal queue intent for that change. Manual merge deferral MUST NOT cause scheduler queue reconciliation to re-dispatch the archived workspace as ordinary queued work.

An explicit `ResolveMerge` command remains the way to retry a manual merge-wait change after the user resolves the manual blocker. If repository-visible evidence still shows an archived-but-not-merged change is waiting for manual merge retry, the reducer MUST accept retry intent in a form that remains scheduler-consumable and MUST NOT silently drop it while the TUI continues to display pending retry.

After a manual `MergeDeferred(auto_resumable=false)` returns a change to `MergeWait`, a later explicit `ResolveMerge` for the same change MUST be treated as a fresh user retry intent. Executor-local retry dedupe, dirty-state tracking, or previous dispatch snapshots MUST NOT suppress this retry after the manual blocker has been resolved.

If an explicit `ResolveMerge` for a `MergeWait` change is accepted after a previous dirty manual deferral, the active or newly-started scheduler MUST consume the same shared reducer-owned `ResolveWait` membership even when there are no ordinary queued apply candidates. A scheduler run started for this purpose MUST NOT replace caller-owned reducer state with a fresh reducer and MUST NOT complete as an ordinary zero-change run while accepted retry membership remains.

After a change has reached repository-visible base integration, later stale duplicate merge outcomes for the same change MUST NOT regress the reducer-visible lifecycle from `Merged` to `MergeWait` or `ResolveWait`.

The shared reducer state that accepts `ResolveMerge` MUST be the same authoritative reducer state observed by the scheduler/executor that consumes the retry. A service or executor construction path MUST NOT replace caller-owned reducer state with a fresh empty reducer state after retry intent has been accepted. State synchronization may copy reducer-owned lane-wait membership into executor-local caches, but the copied cache MUST NOT become an independent source of truth that can make the UI show `resolve pending` after reducer-owned membership has been cleared.

<!-- Expected canonical result after archive: `orchestration-state` will require explicit manual retry after dirty manual deferral to be consumed by the same shared scheduler/reducer path even when normal queue work is empty. -->

#### Scenario: dirty base manual retry returns to merge wait

**Given**: change `alpha` is archive-complete and ready for post-archive merge handling
**And**: the base repository has uncommitted changes
**When**: the user requests merge retry for `alpha`
**Then**: the merge attempt is deferred with `MergeDeferred(alpha, auto_resumable=false)`
**And**: reducer-visible status for `alpha` becomes `merge wait`
**And**: `alpha` is removed from reducer-owned resolve-wait membership
**And**: `alpha` is not reintroduced as ordinary queued apply work

#### Scenario: explicit retry after base clean starts scheduler-consumed retry

**Given**: change `alpha` is in reducer-visible `merge wait` due to dirty-base manual deferral
**And**: `alpha` still has an archive-complete workspace that is not merged to the base branch
**And**: the base repository has become clean
**When**: the user requests merge retry for `alpha` again
**Then**: `ReducerCommand::ResolveMerge(alpha)` is accepted
**And**: the reducer records `alpha` in `ResolveWait`
**And**: the active or newly-started scheduler consumes that same reducer-owned `ResolveWait` intent
**And**: retry evaluation reaches the merge attempt path for `alpha`
**And**: if no blocker remains, `alpha` can transition to `merged`

#### Scenario: clean retry is not suppressed by prior dirty dispatch

**Given**: change `alpha` previously had a manual retry dispatched
**And**: that retry returned to `MergeWait` through dirty workspace/base manual deferral
**And**: executor-local retry dispatch snapshots or dirty-state observations still remember the previous attempt
**And**: the base/workspace retry preconditions are now clean
**When**: the user requests merge retry for `alpha` again
**Then**: stale executor-local state does not suppress retry dispatch
**And**: retry evaluation uses current workspace and base repository state
**And**: retry evaluation reaches the merge attempt path for `alpha`

#### Scenario: zero-normal-queue retry consumes shared reducer membership

**Given**: no ordinary queued apply candidates exist
**And**: no scheduler is currently running
**And**: the shared reducer accepts `ResolveMerge(alpha)` for a manual `merge wait` row
**When**: the TUI command handler starts a scheduler run for the manual retry
**Then**: the scheduler observes the caller-owned shared reducer state containing `ResolveWait(alpha)`
**And**: the scheduler evaluates base-lane waiters before reporting ordinary zero-change completion
**And**: `alpha` leaves `ResolveWait` only through scheduler-owned events or visible blocker evidence

#### Scenario: live scheduler notification consumes lane waiter without ordinary queue work

**Given**: the TUI logs `Scheduled merge-wait retry intent for 'alpha'; notified existing scheduler`
**And**: the scheduler is already running
**And**: there are no ordinary queued apply candidates remaining
**When**: reducer-owned `ResolveWait(alpha)` exists
**Then**: the scheduler wakes and evaluates base-lane waiters
**And**: the retry does not require another queued change or another user keypress to make progress

#### Scenario: explicit manual retry is not suppressed by stale dispatch dedupe

**Given**: change `alpha` previously had a `ResolveWait` retry dispatched
**And**: that retry returned to manual `merge wait` through `MergeDeferred(alpha, auto_resumable=false)`
**When**: the user resolves the manual blocker and requests merge retry for `alpha` again
**Then**: stale executor-local dispatch snapshots or dirty-state observations do not suppress retry dispatch
**And**: the retry is evaluated against current workspace and base repository state

#### Scenario: accepted retry emits evidence when still blocked

**Given**: explicit retry for `alpha` is accepted into reducer-owned `ResolveWait`
**And**: retry evaluation still cannot start or complete merge handling
**When**: the scheduler evaluates the retry
**Then**: the system emits log or event evidence identifying the remaining blocker
**And**: `alpha` remains in the correct reducer-visible wait state for that blocker

### Requirement: Reducer-Owned Change Runtime State

Reducer-owned orchestration state SHALL reflect the latest repository-visible terminal outcome for a change. A recoverable error terminal state SHALL NOT remain the current display state after the same change later emits archive, merge, push, or resolve success events.

Success events MAY supersede `TerminalState::Error` for the same change because errors from acceptance, apply, archive, resolve, or push attempts are recoverable until the change reaches the terminal success required by its invocation mode or final rejection. Success events MUST NOT overwrite final rejection state.

Without opt-in upstream integration, successful cumulative base integration SHALL transition a parallel change to terminal `Merged`. With opt-in upstream integration, local cumulative base integration SHALL remain non-terminal publication progress, and only change-scoped `PushCompleted` emitted after selected-remote observation confirms cumulative HEAD reachability SHALL transition the change to terminal `Pushed`. An opted-in change MUST NOT be displayed as final `merged` while publication remains pending, failed, stalled, or unconfirmed.

A recoverable error terminal state MUST gate ordinary apply dispatch. Explicit retry MUST be limited to recoverable work and MUST NOT requeue final rejected, merged, pushed, or archived terminal states. Retry of an opted-in locally integrated but unpublished change MUST resume upstream publication and MUST NOT create ordinary apply or acceptance dispatch. In persistent local TUI, a publication failure or stall that exhausts its bounded automatic cycle MUST project into the existing operator-visible recoverable error flow, where F5 or the equivalent local web-control retry starts explicit publication retry. The base lane MUST remain closed to later completed-result integration until remote confirmation succeeds or the operator stops orchestration.

#### Scenario: disabled cumulative merge becomes merged terminal

**Given**: change `alpha` completes cumulative base integration without upstream integration enabled
**When**: the reducer receives merge success
**Then**: terminal state becomes `Merged`
**And**: display status is `merged`

#### Scenario: opted-in local merge remains non-terminal

**Given**: change `alpha` is running with upstream integration enabled
**When**: its archived result merges successfully into cumulative base
**Then**: reducer-owned state records publication progress without terminal `Merged`
**And**: the display does not claim final `merged` success
**And**: ordinary apply and acceptance dispatch for `alpha` remain disabled

#### Scenario: remote-confirmed publication becomes pushed terminal

**Given**: change `alpha` is locally integrated with upstream integration enabled
**And**: Conflux confirms through remote observation that cumulative HEAD is reachable from the selected remote base
**When**: `alpha` receives change-scoped `PushCompleted`
**Then**: terminal state becomes `Pushed`
**And**: display status is `pushed`
**And**: `alpha` is not displayed as `merged`

#### Scenario: publication failure remains resumable

**Given**: change `alpha` is locally integrated with upstream integration enabled
**When**: verification, push, or remote confirmation fails
**Then**: `alpha` does not become `Merged` or `Pushed`
**And**: reducer-owned state exposes recoverable publication failure or wait evidence
**And**: explicit retry returns `alpha` to publication work rather than ordinary apply work

#### Scenario: persistent TUI retries failed publication

**Given**: local TUI owns publication for change `alpha`
**And**: the bounded publication cycle has ended in a recoverable failure or stall
**When**: the TUI projects its reducer-owned state
**Then**: it displays `alpha` through the existing recoverable Error-mode interaction
**And**: F5 or the equivalent local web-control action is available as explicit retry
**And**: later completed results remain waiting before base integration
**And**: no ordinary apply or acceptance dispatch for `alpha` is created

#### Scenario: successful TUI retry releases waiting base integration

**Given**: local TUI displays recoverable publication failure for change `alpha`
**And**: change `beta` is waiting before cumulative-base integration
**When**: the operator explicitly retries and Conflux remotely confirms `alpha`
**Then**: `alpha` becomes terminal `Pushed`
**And**: the base lane becomes available for `beta`
**And**: the TUI remains active

#### Scenario: late publication success supersedes recoverable failure

**Given**: change `alpha` has recoverable publication error state
**And**: already-running or retried repository work later confirms cumulative HEAD on the selected remote
**When**: `PushCompleted(alpha)` arrives
**Then**: terminal state becomes `Pushed`
**And**: no ordinary apply dispatch is created

#### Scenario: pushed terminal is not retryable

**Given**: change `alpha` has terminal state `Pushed`
**When**: an explicit retry transition is requested
**Then**: `alpha` remains `Pushed`
**And**: it is not reintroduced as apply, acceptance, merge, or publication work

### Requirement: Rejection Flow Execution

The system SHALL execute the existing rejection flow when acceptance returns a `Blocked` verdict. The sole managed-worktree execution service SHALL own this flow and preserve its repository-verifiable marker, commit isolation, and worktree cleanup behavior.

#### Scenario: Rejection flow commits only REJECTED.md and cleans worktree

- **GIVEN** acceptance has returned `Blocked` for change `fix-auth`
- **WHEN** the worktree execution service runs rejection handling
- **THEN** `openspec/changes/fix-auth/REJECTED.md` is created with the rejection reason
- **AND** the base commit includes only that marker
- **AND** the rejected worktree is deleted

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

An archived managed-worktree change MUST enter reducer-owned post-archive handling. It MUST use `MergeWait` only for a concrete deferred condition that requires waiting or user intervention; recoverable internal preconditions MUST proceed automatically.

#### Scenario: archived change does not stay merge wait for recoverable branch initialization

- **GIVEN** an archived change enters merge handling
- **AND** the Git base branch has not yet been cached
- **WHEN** the system can initialize that base branch from repository state
- **THEN** the change proceeds through merge handling
- **AND** it does not remain in `MergeWait`

### Requirement: Rejection Flow Execution

The system SHALL execute the existing rejection flow when acceptance returns a `Blocked` verdict. The sole managed-worktree execution service SHALL own this flow and preserve its repository-verifiable marker, commit isolation, and worktree cleanup behavior.

#### Scenario: Rejection flow commits only REJECTED.md and cleans worktree

- **GIVEN** acceptance has returned `Blocked` for change `fix-auth`
- **WHEN** the worktree execution service runs rejection handling
- **THEN** `openspec/changes/fix-auth/REJECTED.md` is created with the rejection reason
- **AND** the base commit includes only that marker
- **AND** the rejected worktree is deleted

### Requirement: Rejection Flow Execution

The system SHALL execute the existing rejection flow when acceptance returns a `Blocked` verdict. The sole managed-worktree execution service SHALL own this flow and preserve its repository-verifiable marker, commit isolation, and worktree cleanup behavior.

#### Scenario: Rejection flow commits only REJECTED.md and cleans worktree

- **GIVEN** acceptance has returned `Blocked` for change `fix-auth`
- **WHEN** the worktree execution service runs rejection handling
- **THEN** `openspec/changes/fix-auth/REJECTED.md` is created with the rejection reason
- **AND** the base commit includes only that marker
- **AND** the rejected worktree is deleted

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

Reducer-owned orchestration state SHALL reflect the latest repository-visible terminal outcome for a change. A recoverable error terminal state SHALL NOT remain the current display state after the same change later emits archive, merge, push, or resolve success events.

Success events MAY supersede `TerminalState::Error` for the same change because errors from acceptance, apply, archive, resolve, or push attempts are recoverable until the change reaches the terminal success required by its invocation mode or final rejection. Success events MUST NOT overwrite final rejection state.

Without opt-in upstream integration, successful cumulative base integration SHALL transition a parallel change to terminal `Merged`. With opt-in upstream integration, local cumulative base integration SHALL remain non-terminal publication progress, and only change-scoped `PushCompleted` emitted after selected-remote observation confirms cumulative HEAD reachability SHALL transition the change to terminal `Pushed`. An opted-in change MUST NOT be displayed as final `merged` while publication remains pending, failed, stalled, or unconfirmed.

A recoverable error terminal state MUST gate ordinary apply dispatch. Explicit retry MUST be limited to recoverable work and MUST NOT requeue final rejected, merged, pushed, or archived terminal states. Retry of an opted-in locally integrated but unpublished change MUST resume upstream publication and MUST NOT create ordinary apply or acceptance dispatch. In persistent local TUI, a publication failure or stall that exhausts its bounded automatic cycle MUST project into the existing operator-visible recoverable error flow, where F5 or the equivalent local web-control retry starts explicit publication retry. The base lane MUST remain closed to later completed-result integration until remote confirmation succeeds or the operator stops orchestration.

#### Scenario: disabled cumulative merge becomes merged terminal

**Given**: change `alpha` completes cumulative base integration without upstream integration enabled
**When**: the reducer receives merge success
**Then**: terminal state becomes `Merged`
**And**: display status is `merged`

#### Scenario: opted-in local merge remains non-terminal

**Given**: change `alpha` is running with upstream integration enabled
**When**: its archived result merges successfully into cumulative base
**Then**: reducer-owned state records publication progress without terminal `Merged`
**And**: the display does not claim final `merged` success
**And**: ordinary apply and acceptance dispatch for `alpha` remain disabled

#### Scenario: remote-confirmed publication becomes pushed terminal

**Given**: change `alpha` is locally integrated with upstream integration enabled
**And**: Conflux confirms through remote observation that cumulative HEAD is reachable from the selected remote base
**When**: `alpha` receives change-scoped `PushCompleted`
**Then**: terminal state becomes `Pushed`
**And**: display status is `pushed`
**And**: `alpha` is not displayed as `merged`

#### Scenario: publication failure remains resumable

**Given**: change `alpha` is locally integrated with upstream integration enabled
**When**: verification, push, or remote confirmation fails
**Then**: `alpha` does not become `Merged` or `Pushed`
**And**: reducer-owned state exposes recoverable publication failure or wait evidence
**And**: explicit retry returns `alpha` to publication work rather than ordinary apply work

#### Scenario: persistent TUI retries failed publication

**Given**: local TUI owns publication for change `alpha`
**And**: the bounded publication cycle has ended in a recoverable failure or stall
**When**: the TUI projects its reducer-owned state
**Then**: it displays `alpha` through the existing recoverable Error-mode interaction
**And**: F5 or the equivalent local web-control action is available as explicit retry
**And**: later completed results remain waiting before base integration
**And**: no ordinary apply or acceptance dispatch for `alpha` is created

#### Scenario: successful TUI retry releases waiting base integration

**Given**: local TUI displays recoverable publication failure for change `alpha`
**And**: change `beta` is waiting before cumulative-base integration
**When**: the operator explicitly retries and Conflux remotely confirms `alpha`
**Then**: `alpha` becomes terminal `Pushed`
**And**: the base lane becomes available for `beta`
**And**: the TUI remains active

#### Scenario: late publication success supersedes recoverable failure

**Given**: change `alpha` has recoverable publication error state
**And**: already-running or retried repository work later confirms cumulative HEAD on the selected remote
**When**: `PushCompleted(alpha)` arrives
**Then**: terminal state becomes `Pushed`
**And**: no ordinary apply dispatch is created

#### Scenario: pushed terminal is not retryable

**Given**: change `alpha` has terminal state `Pushed`
**When**: an explicit retry transition is requested
**Then**: `alpha` remains `Pushed`
**And**: it is not reintroduced as apply, acceptance, merge, or publication work

### Requirement: Rejected terminal state remains distinct from errors

The terminal result MUST include `Rejected` as a permanent terminal state distinct from `Error`. A rejected change is one where rejecting review has confirmed the specification is unimplementable or otherwise out of scope for completion, requiring a rollback to the base branch with a documented reason.

Acceptance-gate and rejecting-review holds that are not confirmed as rejected MUST remain non-terminal. A validated non-repository prerequisite MUST display as `blocked`; no-progress, repeated-finding, exhausted-policy, and other intervention holds that do not validate as external prerequisites MUST display as `stalled`.

#### Scenario: rejecting-confirmed change becomes rejected terminal state

- **GIVEN** a change is in `Rejecting`
- **AND** the rejection flow completes (`REJECTED.md` committed and worktree removed)
- **WHEN** the reducer applies the terminal rejection event
- **THEN** the terminal result becomes `Rejected`
- **AND** the derived display status is `rejected`

#### Scenario: validated external acceptance hold remains blocked

- **GIVEN** acceptance reports a validated non-repository prerequisite
- **AND** rejecting review has not confirmed terminal rejection
- **WHEN** the reducer exposes the paused lifecycle state
- **THEN** the terminal result remains `None`
- **AND** the derived display status is `blocked`

#### Scenario: non-external acceptance hold remains stalled

- **GIVEN** acceptance stops after no semantic progress, repeated findings, or exhausted repair policy
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

Reducer-owned orchestration state SHALL reflect the latest repository-visible terminal outcome for a change. A recoverable error terminal state SHALL NOT remain the current display state after the same change later emits archive, merge, push, or resolve success events.

Success events MAY supersede `TerminalState::Error` for the same change because errors from acceptance, apply, archive, resolve, or push attempts are recoverable until the change reaches the terminal success required by its invocation mode or final rejection. Success events MUST NOT overwrite final rejection state.

Without opt-in upstream integration, successful cumulative base integration SHALL transition a parallel change to terminal `Merged`. With opt-in upstream integration, local cumulative base integration SHALL remain non-terminal publication progress, and only change-scoped `PushCompleted` emitted after selected-remote observation confirms cumulative HEAD reachability SHALL transition the change to terminal `Pushed`. An opted-in change MUST NOT be displayed as final `merged` while publication remains pending, failed, stalled, or unconfirmed.

A recoverable error terminal state MUST gate ordinary apply dispatch. Explicit retry MUST be limited to recoverable work and MUST NOT requeue final rejected, merged, pushed, or archived terminal states. Retry of an opted-in locally integrated but unpublished change MUST resume upstream publication and MUST NOT create ordinary apply or acceptance dispatch. In persistent local TUI, a publication failure or stall that exhausts its bounded automatic cycle MUST project into the existing operator-visible recoverable error flow, where F5 or the equivalent local web-control retry starts explicit publication retry. The base lane MUST remain closed to later completed-result integration until remote confirmation succeeds or the operator stops orchestration.

#### Scenario: disabled cumulative merge becomes merged terminal

**Given**: change `alpha` completes cumulative base integration without upstream integration enabled
**When**: the reducer receives merge success
**Then**: terminal state becomes `Merged`
**And**: display status is `merged`

#### Scenario: opted-in local merge remains non-terminal

**Given**: change `alpha` is running with upstream integration enabled
**When**: its archived result merges successfully into cumulative base
**Then**: reducer-owned state records publication progress without terminal `Merged`
**And**: the display does not claim final `merged` success
**And**: ordinary apply and acceptance dispatch for `alpha` remain disabled

#### Scenario: remote-confirmed publication becomes pushed terminal

**Given**: change `alpha` is locally integrated with upstream integration enabled
**And**: Conflux confirms through remote observation that cumulative HEAD is reachable from the selected remote base
**When**: `alpha` receives change-scoped `PushCompleted`
**Then**: terminal state becomes `Pushed`
**And**: display status is `pushed`
**And**: `alpha` is not displayed as `merged`

#### Scenario: publication failure remains resumable

**Given**: change `alpha` is locally integrated with upstream integration enabled
**When**: verification, push, or remote confirmation fails
**Then**: `alpha` does not become `Merged` or `Pushed`
**And**: reducer-owned state exposes recoverable publication failure or wait evidence
**And**: explicit retry returns `alpha` to publication work rather than ordinary apply work

#### Scenario: persistent TUI retries failed publication

**Given**: local TUI owns publication for change `alpha`
**And**: the bounded publication cycle has ended in a recoverable failure or stall
**When**: the TUI projects its reducer-owned state
**Then**: it displays `alpha` through the existing recoverable Error-mode interaction
**And**: F5 or the equivalent local web-control action is available as explicit retry
**And**: later completed results remain waiting before base integration
**And**: no ordinary apply or acceptance dispatch for `alpha` is created

#### Scenario: successful TUI retry releases waiting base integration

**Given**: local TUI displays recoverable publication failure for change `alpha`
**And**: change `beta` is waiting before cumulative-base integration
**When**: the operator explicitly retries and Conflux remotely confirms `alpha`
**Then**: `alpha` becomes terminal `Pushed`
**And**: the base lane becomes available for `beta`
**And**: the TUI remains active

#### Scenario: late publication success supersedes recoverable failure

**Given**: change `alpha` has recoverable publication error state
**And**: already-running or retried repository work later confirms cumulative HEAD on the selected remote
**When**: `PushCompleted(alpha)` arrives
**Then**: terminal state becomes `Pushed`
**And**: no ordinary apply dispatch is created

#### Scenario: pushed terminal is not retryable

**Given**: change `alpha` has terminal state `Pushed`
**When**: an explicit retry transition is requested
**Then**: `alpha` remains `Pushed`
**And**: it is not reintroduced as apply, acceptance, merge, or publication work

### Requirement: Reducer Input Precedence and Idempotency

Workspace observations and refresh-derived archive-complete evidence SHALL NOT regress reducer-owned active, pending, or terminal lifecycle states to `MergeWait` without concrete manual deferral evidence.

A `ChangesRefreshed` event containing a change in `merge_wait_ids` represents archived-but-not-yet-merged workspace evidence. That evidence MAY preserve or restore an already-established manual `MergeWait`, but it MUST NOT override `ActivityState::Resolving`, `WaitState::ResolveWait`, `WaitState::RejectWait`, `ActivityState::Rejecting`, or terminal states.

<!-- Expected canonical result after archive: `orchestration-state` will treat refresh-derived merge-wait evidence as lower precedence than reducer-owned active/pending/terminal state. -->

#### Scenario: refresh evidence does not regress resolving

**Given**: change `alpha` has `ActivityState::Resolving`
**When**: a `ChangesRefreshed` event includes `alpha` in `merge_wait_ids`
**Then**: `alpha` remains `resolving`
**And**: `alpha` is not changed to `merge wait`

#### Scenario: refresh evidence does not regress resolve pending

**Given**: change `alpha` has `WaitState::ResolveWait`
**When**: a `ChangesRefreshed` event includes `alpha` in `merge_wait_ids`
**Then**: `alpha` remains `resolve pending`
**And**: reducer-owned scheduler retry membership remains available

#### Scenario: refresh evidence can preserve concrete manual merge wait

**Given**: change `alpha` has already received concrete manual deferral evidence and is in `WaitState::MergeWait`
**When**: a `ChangesRefreshed` event includes `alpha` in `merge_wait_ids`
**Then**: `alpha` remains `merge wait`
**And**: no normal queue intent is reintroduced for `alpha`

### Requirement: Reducer-Owned Change Runtime State

Reducer-owned orchestration state SHALL reflect the latest repository-visible terminal outcome for a change. A recoverable error terminal state SHALL NOT remain the current display state after the same change later emits archive, merge, push, or resolve success events.

Success events MAY supersede `TerminalState::Error` for the same change because errors from acceptance, apply, archive, resolve, or push attempts are recoverable until the change reaches the terminal success required by its invocation mode or final rejection. Success events MUST NOT overwrite final rejection state.

Without opt-in upstream integration, successful cumulative base integration SHALL transition a parallel change to terminal `Merged`. With opt-in upstream integration, local cumulative base integration SHALL remain non-terminal publication progress, and only change-scoped `PushCompleted` emitted after selected-remote observation confirms cumulative HEAD reachability SHALL transition the change to terminal `Pushed`. An opted-in change MUST NOT be displayed as final `merged` while publication remains pending, failed, stalled, or unconfirmed.

A recoverable error terminal state MUST gate ordinary apply dispatch. Explicit retry MUST be limited to recoverable work and MUST NOT requeue final rejected, merged, pushed, or archived terminal states. Retry of an opted-in locally integrated but unpublished change MUST resume upstream publication and MUST NOT create ordinary apply or acceptance dispatch. In persistent local TUI, a publication failure or stall that exhausts its bounded automatic cycle MUST project into the existing operator-visible recoverable error flow, where F5 or the equivalent local web-control retry starts explicit publication retry. The base lane MUST remain closed to later completed-result integration until remote confirmation succeeds or the operator stops orchestration.

#### Scenario: disabled cumulative merge becomes merged terminal

**Given**: change `alpha` completes cumulative base integration without upstream integration enabled
**When**: the reducer receives merge success
**Then**: terminal state becomes `Merged`
**And**: display status is `merged`

#### Scenario: opted-in local merge remains non-terminal

**Given**: change `alpha` is running with upstream integration enabled
**When**: its archived result merges successfully into cumulative base
**Then**: reducer-owned state records publication progress without terminal `Merged`
**And**: the display does not claim final `merged` success
**And**: ordinary apply and acceptance dispatch for `alpha` remain disabled

#### Scenario: remote-confirmed publication becomes pushed terminal

**Given**: change `alpha` is locally integrated with upstream integration enabled
**And**: Conflux confirms through remote observation that cumulative HEAD is reachable from the selected remote base
**When**: `alpha` receives change-scoped `PushCompleted`
**Then**: terminal state becomes `Pushed`
**And**: display status is `pushed`
**And**: `alpha` is not displayed as `merged`

#### Scenario: publication failure remains resumable

**Given**: change `alpha` is locally integrated with upstream integration enabled
**When**: verification, push, or remote confirmation fails
**Then**: `alpha` does not become `Merged` or `Pushed`
**And**: reducer-owned state exposes recoverable publication failure or wait evidence
**And**: explicit retry returns `alpha` to publication work rather than ordinary apply work

#### Scenario: persistent TUI retries failed publication

**Given**: local TUI owns publication for change `alpha`
**And**: the bounded publication cycle has ended in a recoverable failure or stall
**When**: the TUI projects its reducer-owned state
**Then**: it displays `alpha` through the existing recoverable Error-mode interaction
**And**: F5 or the equivalent local web-control action is available as explicit retry
**And**: later completed results remain waiting before base integration
**And**: no ordinary apply or acceptance dispatch for `alpha` is created

#### Scenario: successful TUI retry releases waiting base integration

**Given**: local TUI displays recoverable publication failure for change `alpha`
**And**: change `beta` is waiting before cumulative-base integration
**When**: the operator explicitly retries and Conflux remotely confirms `alpha`
**Then**: `alpha` becomes terminal `Pushed`
**And**: the base lane becomes available for `beta`
**And**: the TUI remains active

#### Scenario: late publication success supersedes recoverable failure

**Given**: change `alpha` has recoverable publication error state
**And**: already-running or retried repository work later confirms cumulative HEAD on the selected remote
**When**: `PushCompleted(alpha)` arrives
**Then**: terminal state becomes `Pushed`
**And**: no ordinary apply dispatch is created

#### Scenario: pushed terminal is not retryable

**Given**: change `alpha` has terminal state `Pushed`
**When**: an explicit retry transition is requested
**Then**: `alpha` remains `Pushed`
**And**: it is not reintroduced as apply, acceptance, merge, or publication work

### Requirement: WebSocket change status consistency with TUI

Server-mode WebSocket API SHALL produce the same set of display status strings as `ChangeRuntimeState.display_status()`. The system MUST NOT maintain a separate mapping from workspace states to display strings that diverges from the reducer-derived status vocabulary. Blocked payloads SHALL expose a machine-readable dependency or external blocker kind and SHALL preserve validated external blocker detail.

#### Scenario: All display statuses are representable in WebSocket payloads

- **GIVEN** the reducer can produce any of: `not queued`, `queued`, `blocked`, `stalled`, `applying`, `accepting`, `rejecting`, `archiving`, `resolving`, `merge wait`, `resolve pending`, `archived`, `pushed`, `merged`, `rejected`, `error`, `stopped`
- **WHEN** a WebSocket client receives a change list
- **THEN** the status field for each change is one of the above values
- **AND** a blocked change carries its blocker kind

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

An explicit `ResolveMerge` command remains the way to retry a manual merge-wait change after the user resolves the manual blocker. If repository-visible evidence still shows an archived-but-not-merged change is waiting for manual merge retry, the reducer MUST accept retry intent in a form that remains scheduler-consumable and MUST NOT silently drop it while the TUI continues to display pending retry.

After a manual `MergeDeferred(auto_resumable=false)` returns a change to `MergeWait`, a later explicit `ResolveMerge` for the same change MUST be treated as a fresh user retry intent. Executor-local retry dedupe, dirty-state tracking, or previous dispatch snapshots MUST NOT suppress this retry after the manual blocker has been resolved.

If an explicit `ResolveMerge` for a `MergeWait` change is accepted after a previous dirty manual deferral, the active or newly-started scheduler MUST consume the same shared reducer-owned `ResolveWait` membership even when there are no ordinary queued apply candidates. A scheduler run started for this purpose MUST NOT replace caller-owned reducer state with a fresh reducer and MUST NOT complete as an ordinary zero-change run while accepted retry membership remains.

After a change has reached repository-visible base integration, later stale duplicate merge outcomes for the same change MUST NOT regress the reducer-visible lifecycle from `Merged` to `MergeWait` or `ResolveWait`.

The shared reducer state that accepts `ResolveMerge` MUST be the same authoritative reducer state observed by the scheduler/executor that consumes the retry. A service or executor construction path MUST NOT replace caller-owned reducer state with a fresh empty reducer state after retry intent has been accepted. State synchronization may copy reducer-owned lane-wait membership into executor-local caches, but the copied cache MUST NOT become an independent source of truth that can make the UI show `resolve pending` after reducer-owned membership has been cleared.

<!-- Expected canonical result after archive: `orchestration-state` will require explicit manual retry after dirty manual deferral to be consumed by the same shared scheduler/reducer path even when normal queue work is empty. -->

#### Scenario: dirty base manual retry returns to merge wait

**Given**: change `alpha` is archive-complete and ready for post-archive merge handling
**And**: the base repository has uncommitted changes
**When**: the user requests merge retry for `alpha`
**Then**: the merge attempt is deferred with `MergeDeferred(alpha, auto_resumable=false)`
**And**: reducer-visible status for `alpha` becomes `merge wait`
**And**: `alpha` is removed from reducer-owned resolve-wait membership
**And**: `alpha` is not reintroduced as ordinary queued apply work

#### Scenario: explicit retry after base clean starts scheduler-consumed retry

**Given**: change `alpha` is in reducer-visible `merge wait` due to dirty-base manual deferral
**And**: `alpha` still has an archive-complete workspace that is not merged to the base branch
**And**: the base repository has become clean
**When**: the user requests merge retry for `alpha` again
**Then**: `ReducerCommand::ResolveMerge(alpha)` is accepted
**And**: the reducer records `alpha` in `ResolveWait`
**And**: the active or newly-started scheduler consumes that same reducer-owned `ResolveWait` intent
**And**: retry evaluation reaches the merge attempt path for `alpha`
**And**: if no blocker remains, `alpha` can transition to `merged`

#### Scenario: clean retry is not suppressed by prior dirty dispatch

**Given**: change `alpha` previously had a manual retry dispatched
**And**: that retry returned to `MergeWait` through dirty workspace/base manual deferral
**And**: executor-local retry dispatch snapshots or dirty-state observations still remember the previous attempt
**And**: the base/workspace retry preconditions are now clean
**When**: the user requests merge retry for `alpha` again
**Then**: stale executor-local state does not suppress retry dispatch
**And**: retry evaluation uses current workspace and base repository state
**And**: retry evaluation reaches the merge attempt path for `alpha`

#### Scenario: zero-normal-queue retry consumes shared reducer membership

**Given**: no ordinary queued apply candidates exist
**And**: no scheduler is currently running
**And**: the shared reducer accepts `ResolveMerge(alpha)` for a manual `merge wait` row
**When**: the TUI command handler starts a scheduler run for the manual retry
**Then**: the scheduler observes the caller-owned shared reducer state containing `ResolveWait(alpha)`
**And**: the scheduler evaluates base-lane waiters before reporting ordinary zero-change completion
**And**: `alpha` leaves `ResolveWait` only through scheduler-owned events or visible blocker evidence

#### Scenario: live scheduler notification consumes lane waiter without ordinary queue work

**Given**: the TUI logs `Scheduled merge-wait retry intent for 'alpha'; notified existing scheduler`
**And**: the scheduler is already running
**And**: there are no ordinary queued apply candidates remaining
**When**: reducer-owned `ResolveWait(alpha)` exists
**Then**: the scheduler wakes and evaluates base-lane waiters
**And**: the retry does not require another queued change or another user keypress to make progress

#### Scenario: explicit manual retry is not suppressed by stale dispatch dedupe

**Given**: change `alpha` previously had a `ResolveWait` retry dispatched
**And**: that retry returned to manual `merge wait` through `MergeDeferred(alpha, auto_resumable=false)`
**When**: the user resolves the manual blocker and requests merge retry for `alpha` again
**Then**: stale executor-local dispatch snapshots or dirty-state observations do not suppress retry dispatch
**And**: the retry is evaluated against current workspace and base repository state

#### Scenario: accepted retry emits evidence when still blocked

**Given**: explicit retry for `alpha` is accepted into reducer-owned `ResolveWait`
**And**: retry evaluation still cannot start or complete merge handling
**When**: the scheduler evaluates the retry
**Then**: the system emits log or event evidence identifying the remaining blocker
**And**: `alpha` remains in the correct reducer-visible wait state for that blocker

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

When a parallel workspace has already moved a change into `openspec/changes/archive/` but archive commit finalization remains incomplete, repository evidence SHALL continue to classify that workspace as recoverable rather than permanently terminal solely because an earlier attempt failed. Workspace file state, Git state, and base-tree comparison remain authoritative for the resume phase, and no durable external resume state is required.

Recoverable evidence alone MUST NOT create current-process operator intent. The scheduler SHALL re-own archived-dirty repair on a later cycle or restarted run only after the change is an explicit invocation target, has current reducer `QueueIntent::Queued`, or has applicable reducer-owned `ResolveWait` or `RejectWait` lane intent. An unrequested archived-dirty workspace MUST NOT prevent an otherwise drained run from becoming idle or complete.

#### Scenario: Restart preserves recoverability without automatic execution

**Given**: `alpha` is archived-dirty and not merged in its preserved workspace
**When**: Conflux restarts with no execution marks, queue intent, or lane-wait intent for `alpha`
**Then**: Workspace evidence still classifies `alpha` as recoverable
**And**: The scheduler does not execute or mutate `alpha`
**And**: An otherwise drained run may remain idle or complete

#### Scenario: Explicit intent reclaims archived-dirty workspace

**Given**: `alpha` is archived-dirty and not merged
**And**: A new invocation explicitly targets `alpha` or accepted reducer queue intent exists for `alpha`
**When**: The scheduler resolves eligible work
**Then**: It reclaims `alpha` as archive-finalization recovery work
**And**: It derives the resumed phase from current repository evidence
**And**: It does not require durable external resume state

#### Scenario: Archived dirty recovery does not require full archive command rerun

**Given**: Archive file movement for `alpha` is already correct
**And**: Only archive commit finalization remains incomplete
**And**: `alpha` has current explicit execution intent
**When**: Conflux resumes recovery for `alpha`
**Then**: It resumes archive finalization repair rather than rerunning the full archive command unnecessarily
**And**: It verifies that archive file state has not regressed

#### Scenario: Revoked intent leaves recoverable evidence untouched

**Given**: `alpha` remains archived-dirty after its ordinary queue intent is removed or dequeued
**When**: A later scheduler cycle sees its preserved worktree
**Then**: The workspace remains recoverable evidence
**And**: It is not re-owned as execution work until explicit intent is supplied again

### Requirement: Stalled blocker metadata

When a change enters non-terminal `stalled`, reducer-owned in-memory state and authoritative blocker evidence MUST preserve operator-facing metadata sufficient to distinguish the stall from dependency blocking, validated external blocking, protocol error, and terminal rejection. Validated non-repository prerequisites MUST use external `blocked` metadata instead of stalled metadata.

Acceptance-generated stalled evidence MUST live in the in-memory `OrchestratorState` only for the lifetime of the current process. It MUST NOT be persisted to `~/.local/state/cflx/acceptance-stalls/` or any other out-of-worktree durable location. Process restart MUST clear all in-memory stall state. When repository evidence shows a complete unarchived Apply revision, Conflux MUST run Acceptance again and MUST NOT infer PASS.

Runtime stalled metadata MAY control dispatch suppression, stalled display, and Acceptance retry preparation only. It MUST NOT establish implementation completion, Acceptance PASS, archive readiness, merge eligibility, or base integration, and its mutation or deletion MUST NOT dirty the managed worktree.

#### Scenario: non-external stalled evidence is process-lifetime only

- **GIVEN** validated no-progress, repeated-finding, or exhausted-policy evidence exists in current in-memory state
- **WHEN** the current Conflux process displays the status
- **THEN** reducer state presents the recorded evidence, resumability, and next action
- **AND** display status remains execution `stalled`
- **AND** the managed worktree remains clean

#### Scenario: validated external evidence does not become stalled metadata

- **GIVEN** Apply or Acceptance supplies a validated non-repository prerequisite
- **WHEN** runtime classifies the evidence
- **THEN** it creates external `blocked` metadata
- **AND** it emits no stalled lifecycle transition for that prerequisite

#### Scenario: restart clears stall and re-runs Acceptance

- **GIVEN** a previously stalled in-memory record is gone after restart
- **AND** repository evidence still shows a complete unarchived Apply revision
- **WHEN** Conflux restarts
- **THEN** it does not reconstruct stalled state, PASS, or archive readiness
- **AND** it routes the change to Acceptance rather than Apply or archive

#### Scenario: bare GATED produces no stalled metadata

- **GIVEN** Acceptance emits bare GATED or legacy blocked compatibility input without valid structured blocker evidence
- **WHEN** runtime handles the result
- **THEN** it records no stalled or external blocked metadata
- **AND** bounded Acceptance protocol retry handles the result

### Requirement: Orchestrator classifies external blockers separately from stalls

Conflux SHALL make the final lifecycle classification from validated structured evidence. A change SHALL be `blocked` when a concrete non-repository prerequisite prevents useful execution and the evidence includes a verifiable unblock condition. A change SHALL be `stalled` when automatic execution stops because of no semantic progress, repeated findings, or exhausted retry or repair policy.

Agents SHALL report blocker facts but SHALL NOT directly assign canonical lifecycle state. Compatibility verdict token text alone SHALL NOT determine the classification.

#### Scenario: Structured external prerequisite becomes blocked

- **GIVEN** Apply or Acceptance reports a supported external prerequisite with non-empty evidence, prerequisite or owner, verifiable unblock condition, next action, and resumability
- **AND** repository-only work or a test double cannot satisfy the prerequisite
- **WHEN** the orchestrator validates the report
- **THEN** the proposal lifecycle status is `blocked`
- **AND** blocker metadata identifies the blocker kind as external and preserves its origin and evidence

#### Scenario: Exhausted execution remains stalled

- **GIVEN** a change repeats the same finding, makes no semantic progress, or exhausts its retry or repair budget
- **WHEN** the orchestrator finalizes the automatic execution outcome
- **THEN** the proposal lifecycle status is `stalled`
- **AND** it is not classified as external `blocked` merely because progress stopped

#### Scenario: Bare compatibility token does not choose lifecycle

- **GIVEN** an agent emits `gated` or legacy `blocked` without complete structured blocker evidence
- **WHEN** the orchestrator parses the compatibility input
- **THEN** bounded protocol correction handles the incomplete output
- **AND** the token text alone creates neither external `blocked` nor `stalled`

### Requirement: Blocked lifecycle preserves blocker kind

A blocked proposal SHALL expose whether it is waiting on a proposal dependency or an external prerequisite. External blocker metadata SHALL include origin, category, evidence, prerequisite or owner, unblock condition, next action, and resumability. Dependency blocking SHALL continue to derive from the proposal graph and SHALL NOT be represented as an external blocker.

Structured blocker classifications SHALL take precedence over lower-fidelity workspace status observations for the same non-terminal change. After `AcceptanceGated` or `ExecutionBlocked` establishes an external blocked state or an Acceptance-owned stalled hold, a later or duplicate generic blocked workspace status SHALL NOT change the wait kind, clear structured blocker metadata, clear Acceptance ownership, or change resumability. A generic blocked workspace status MAY establish conservative stalled metadata only when no structured blocker classification already owns the wait.

#### Scenario: Dependency and external waits share status without losing meaning

- **GIVEN** one proposal waits on an unarchived dependency and another waits on a validated external prerequisite
- **WHEN** operator-facing state is rendered
- **THEN** both lifecycle statuses are `blocked`
- **AND** their blocker kinds are respectively dependency and external
- **AND** the external wait does not create a synthetic proposal dependency edge

#### Scenario: Acceptance external blocker survives generic blocked status

- **GIVEN** Acceptance reports a validated resumable external prerequisite with complete structured metadata
- **AND** the reducer classifies the change as external `blocked`
- **WHEN** the corresponding workspace later reports generic `Blocked`
- **THEN** the change remains external `blocked`
- **AND** category, evidence detail, origin, prerequisite owner, unblock condition, next action, and resumability remain unchanged
- **AND** the generic observation does not replace the hold with generic stalled metadata

#### Scenario: Apply external blocker receives the same precedence

- **GIVEN** Apply reports a validated external prerequisite through `ExecutionBlocked`
- **AND** the reducer classifies the change as external `blocked`
- **WHEN** a generic blocked workspace observation follows
- **THEN** the external blocker kind and Apply origin remain intact
- **AND** queue and operator projections continue to use the structured evidence

#### Scenario: Acceptance-owned execution stall survives generic blocked status

- **GIVEN** Acceptance reaches a structured non-external execution hold that is resumable
- **AND** the reducer records an Acceptance-owned `stalled` wait
- **WHEN** the workspace later reports generic `Blocked`
- **THEN** the change remains Acceptance-owned and `stalled`
- **AND** its resumability and structured guidance remain intact
- **AND** ordinary dispatch remains suppressed until explicit retry or process restart

#### Scenario: Generic blocked status remains a conservative fallback

- **GIVEN** a non-terminal change has no structured blocker classification
- **WHEN** its workspace reports generic `Blocked`
- **THEN** the reducer records the existing conservative generic `stalled` fallback
- **AND** it does not invent an external blocker kind, owner, origin, or unblock condition

#### Scenario: Preserved resumability controls explicit retry

- **GIVEN** a structured blocker has survived a later generic blocked workspace status
- **WHEN** an operator requests retry
- **THEN** a resumable hold follows the existing blocked-phase retry route
- **AND** a non-resumable hold is refused without discarding its blocker evidence

### Requirement: External blocked state remains workspace-local and process-lifetime

External blocked runtime state SHALL remain in memory for the current process and SHALL NOT introduce out-of-worktree durable workflow control. Restart routing SHALL be recomputed from workspace file state, workspace git state, and base-branch comparison. Runtime blocker metadata SHALL NOT establish implementation completion, Acceptance PASS, archive readiness, merge eligibility, or integration.

#### Scenario: Restart does not trust blocked runtime state

- **GIVEN** a process held a validated external blocked state
- **WHEN** Conflux restarts with the same workspace and git evidence
- **THEN** previous in-memory status has no routing authority
- **AND** Conflux re-evaluates the current repository evidence
- **AND** a complete unarchived Apply revision returns to Acceptance rather than inferring PASS or archive readiness

#### Scenario: Explicit retry reruns the blocked phase

- **GIVEN** a proposal is externally blocked
- **WHEN** an operator requests retry
- **THEN** Conflux validates current workspace identity
- **AND** a matching workspace is dispatched to rerun the blocked phase without requiring an external-state oracle
- **AND** the new execution result supplies the evidence for clearing or restoring the blocked classification
- **AND** an identity mismatch prevents unsafe dispatch without silently discarding prior blocker evidence

#### Scenario: Dependent proposal waits on an externally blocked dependency

- **GIVEN** proposal `beta` depends on proposal `alpha`
- **AND** `alpha` is externally blocked
- **WHEN** scheduler eligibility is derived
- **THEN** `beta` remains `blocked` with dependency blocker kind
- **AND** `beta` does not inherit `alpha`'s external blocker kind
- **AND** unrelated ready proposals remain eligible

### Requirement: Global stopped event reconciles interrupted runtime state

After an operator-requested run reaches its cancellation-safe terminal boundary, process-level `ExecutionEvent::Stopped` SHALL reconcile reducer-owned transient state before frontend projection. Every non-terminal change that still has active activity, queued intent, or a wait/hold owned by the ending run MUST become idle, `NotQueued`, non-waiting, non-terminal, and protected from stale lifecycle reactivation until explicit requeue. The transition MUST clear blocker metadata, commit-phase presentation, and scheduler-owned resolve/reject/stall membership associated with the stopped run.

The global event MUST NOT assign per-change `TerminalState::Stopped`. It MUST preserve execution marks, workspace and Git evidence, task progress, fresh idle rows, and existing terminal outcomes. All state introduced by this reconciliation SHALL remain process-local and MUST NOT become restart-routing evidence.

#### Scenario: Accepting change returns to resumable not queued state

- **GIVEN** change `alpha` is non-terminal, execution-marked, and reducer-visible as `accepting`
- **WHEN** the reducer receives process-level `Stopped` after scheduler cleanup completes
- **THEN** `alpha` has idle activity, `NotQueued` intent, no wait or blocker metadata, and no per-change terminal outcome
- **AND** its derived display status is `not queued`
- **AND** its execution mark remains set

#### Scenario: Queued and waiting work is released from the stopped run

- **GIVEN** non-terminal changes carry queued intent, dependency or external blocker state, stalled state, merge wait, resolve pending, reject pending, or another active execution stage
- **WHEN** process-level `Stopped` is reduced
- **THEN** each row owned by the ending run becomes idle `not queued`
- **AND** scheduler-owned resolve, reject, and stall membership from that run is cleared
- **AND** no row becomes terminal `stopped`

#### Scenario: Terminal and unrelated fresh rows are preserved

- **GIVEN** one row is already terminal `error`, `merged`, `pushed`, or `rejected`
- **AND** another row is fresh idle `not queued` with no wait or activity
- **WHEN** process-level `Stopped` is reduced
- **THEN** both rows remain unchanged
- **AND** the stop does not erase a change outcome or claim ownership of unrelated idle work

#### Scenario: Same-process reactivation input cannot resurrect stopped work

- **GIVEN** `alpha` was reconciled by process-level `Stopped`
- **WHEN** a late activity event arrives or `ChangesRefreshed.merge_wait_ids` observes its archived workspace in the same process
- **THEN** `alpha` remains `not queued` and does not re-enter `MergeWait`
- **WHEN** the operator later explicitly queues the marked change
- **THEN** the dequeue guard is released and ordinary workspace-derived execution becomes eligible again
- **WHEN** the process instead restarts with the same workspace evidence
- **THEN** startup may re-derive `MergeWait` because the process-local guard is not durable routing state

#### Scenario: Frontends publish one reconciled state

- **GIVEN** TUI and `/api/v2` consume an authoritative dispatch for process-level `Stopped`
- **WHEN** the event is projected and a later change refresh occurs
- **THEN** both surfaces report the interrupted row as `not queued` with `NotQueued` queue intent
- **AND** neither surface restores the preceding active status
- **AND** duplicate `Stopped` delivery does not create another state transition
