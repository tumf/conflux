### Requirement: TUI ステータス表示は Reducer から導出される

TUI の Change ステータス表示（文字列・色）は reducer-derived display status を最優先に同期しなければならない（MUST）。Refresh-time `merge_wait_ids` は archived-but-not-yet-merged workspace evidence から導出された display synchronization hint として扱ってよい（MAY）が、reducer-owned active, pending, or terminal status を `merge wait` に降格してはならない（MUST NOT）。

Specifically, refresh-derived `merge_wait_ids` MUST NOT overwrite reducer-derived `resolving`, `resolve pending`, `rejecting`, `reject pending`, `merged`, `rejected`, or `error` for the same change. It MAY correct stale display-only rows only when the reducer snapshot does not own one of those stronger lifecycle states for the same change.

TUI display caches remain non-authoritative observability state and MUST NOT be used as scheduler dispatch, resume routing, acceptance, archive, or next-action decision inputs.

Stale archive lifecycle events MUST NOT regress a row that is already displayed as `merged` back to `archiving`. Archive-start display updates MAY mark non-terminal rows as `archiving`, but MUST preserve reducer-owned terminal success display when a stale event arrives after merge completion.

<!-- Expected canonical result after archive: `tui-state` will require stale archive-start events to preserve merged display status, in addition to protecting reducer-owned terminal states from refresh-derived merge-wait regressions. -->

#### Scenario: refresh-derived merge wait does not overwrite resolving

**Given**: change `alpha` is displayed as `resolving`
**And**: the reducer snapshot reports `alpha` as `resolving`
**And**: the refresh loop observes `alpha` as archive-complete but not merged into base
**When**: the TUI handles `OrchestratorEvent::ChangesRefreshed` with `alpha` in `merge_wait_ids`
**Then**: `alpha` remains displayed as `resolving`
**And**: the row is not temporarily reverted to `merge wait`

#### Scenario: refresh-derived merge wait does not overwrite accepted manual resolve pending

**Given**: change `alpha` is displayed as `resolve pending`
**And**: the reducer snapshot reports `alpha` as `resolve pending` because scheduler-owned retry intent exists
**And**: the refresh loop observes `alpha` as archive-complete but not merged into base
**When**: the TUI handles `OrchestratorEvent::ChangesRefreshed` with `alpha` in `merge_wait_ids`
**Then**: `alpha` remains displayed as `resolve pending`
**And**: the row is not reverted to `merge wait` solely from refresh-derived evidence

#### Scenario: refresh-derived merge wait does not overwrite terminal or error states

**Given**: change `alpha` is displayed as `merged`, `rejected`, or `error`
**When**: the TUI handles a stale `OrchestratorEvent::ChangesRefreshed` that includes `alpha` in `merge_wait_ids`
**Then**: `alpha` remains displayed as its reducer-derived state
**And**: the row is not regressed to `merge wait`

#### Scenario: refresh-derived merge wait corrects stale display-only pending

**Given**: change `alpha` is displayed locally as `resolve pending`
**And**: the reducer snapshot does not report `alpha` as `resolving`, `resolve pending`, `rejecting`, `reject pending`, `merged`, `rejected`, or `error`
**And**: the refresh loop observes `alpha` as archive-complete but not merged into base
**When**: the TUI handles `OrchestratorEvent::ChangesRefreshed` with `alpha` in `merge_wait_ids`
**Then**: `alpha` may be displayed as `merge wait`
**And**: the display correction does not enqueue, dispatch, archive, accept, or otherwise route workflow execution

#### Scenario: stale archive start does not overwrite merged display

**Given**: change `alpha` is displayed as `merged`
**And**: a stale archive lifecycle event is received for `alpha`
**When**: the TUI handles `OrchestratorEvent::ArchiveStarted` for `alpha`
**Then**: `alpha` remains displayed as `merged`
**And**: the row is not regressed to `archiving`
**And**: this display protection does not enqueue, dispatch, archive, accept, or otherwise route workflow execution

### Requirement: is_resolving scope limitation

`is_resolving` フラグは resolve 操作同士の直列化ガードとしてのみ機能しなければならない（`Resolving` は Change レベルの `ActivityState` であり、Project レベルのロックではない）。同一 Project 内の他の Change に対する apply/accept/archive パイプラインの開始・再開・リトライをブロックしてはならない。

#### Scenario: start_processing succeeds during resolving

- **GIVEN** 同一 Project 内のある Change が `Resolving` 状態である（`is_resolving` が `true`）
- **WHEN** ユーザーが他の Change に対して `start_processing` を実行する
- **THEN** 選択された Change のキュー追加と処理開始が正常に行われる

#### Scenario: resume_processing succeeds during resolving

- **GIVEN** 同一 Project 内のある Change が `Resolving` 状態であり、`AppMode` が `Stopped` である
- **WHEN** ユーザーが `resume_processing` を実行する
- **THEN** マークされた Change が Queued に遷移し処理が再開される

#### Scenario: retry_error_changes succeeds during resolving

- **GIVEN** 同一 Project 内のある Change が `Resolving` 状態であり、`AppMode` が `Error` である
- **WHEN** ユーザーが `retry_error_changes` を実行する
- **THEN** エラー状態の Change が Queued にリセットされリトライが開始される

#### Scenario: request_merge still serialized during resolving

- **GIVEN** 同一 Project 内のある Change が `Resolving` 状態である（`is_resolving` が `true`）
- **WHEN** ユーザーが MergeWait の別の Change に対して M キーを押す
- **THEN** その Change は `resolve_queue` に追加され即時開始はされない（resolve 直列化は維持）

### Requirement: TUI rejected row is visible but not selectable

When a change directory contains both `openspec/changes/<change_id>/proposal.md` and `openspec/changes/<change_id>/REJECTED.md`, the TUI change list SHALL display that change as a read-only `rejected` row rather than omitting it entirely.

A rejected row SHALL NOT participate in execution mark, queue, or resume controls. The TUI MUST keep its frontend-visible execution mark cleared (`selected = false`), MUST ignore queue-oriented key operations for that row, MUST NOT label the row with the `NEW` badge, and MUST visibly present the row's terminal status as `rejected` in both Select and Running mode.

Rejected row discovery during local TUI auto-refresh MUST use the same captured repository root as active change discovery. It MUST NOT depend on ambient process current working directory after TUI startup.

<!-- Expected canonical result after archive: `tui-state` will state that rejected marker row discovery is repo-root based during local TUI refresh and still never produces NEW badges or queue intent. -->

#### Scenario: Rejected change is shown in TUI list

- **GIVEN** `openspec/changes/fix-auth/proposal.md` exists
- **AND** `openspec/changes/fix-auth/REJECTED.md` exists
- **WHEN** the TUI refreshes its change list
- **THEN** `fix-auth` is displayed in the list
- **AND** its display status is `rejected`

#### Scenario: Rejected row cannot gain an execution mark

- **GIVEN** `fix-auth` is displayed as a `rejected` row in the TUI
- **WHEN** the user presses Space on that row
- **THEN** the row remains `selected = false`
- **AND** no x mark is shown for `fix-auth`
- **AND** the display status remains `rejected`

#### Scenario: Rejected row is ignored by queue-oriented actions

- **GIVEN** `fix-auth` is displayed as a `rejected` row in the TUI
- **WHEN** the user invokes queue or resume-oriented actions such as `@` or `F5`
- **THEN** `fix-auth` is not added to the execution queue
- **AND** no execution start is requested for `fix-auth`

#### Scenario: Select mode shows rejected status label

- **GIVEN** `fix-auth` is displayed as a `rejected` row in the TUI Select mode
- **WHEN** the change list row is rendered
- **THEN** the row visibly includes the label `[rejected]`
- **AND** the row does NOT show the `NEW` badge

#### Scenario: Running mode keeps rejected status label

- **GIVEN** `fix-auth` is displayed as a `rejected` row in the TUI Running mode
- **WHEN** the change list row is rendered
- **THEN** the row visibly includes the label `[rejected]`

#### Scenario: Marker removal reactivates the change as unselected active row

- **GIVEN** `fix-auth` was previously shown as a `rejected` row
- **AND** the user removes `openspec/changes/fix-auth/REJECTED.md` from the base branch
- **WHEN** the TUI refreshes after `fix-auth` reappears in the active listing
- **THEN** `fix-auth` is shown as `not queued`
- **AND** `fix-auth` remains `selected = false` until explicitly marked again

#### Scenario: Rejected row refresh uses captured repository root

- **GIVEN** local TUI mode started from repository root `/repo`
- **AND** the process current working directory later differs from `/repo`
- **AND** `/repo/openspec/changes/rejected-visible/proposal.md` exists
- **AND** `/repo/openspec/changes/rejected-visible/REJECTED.md` exists
- **WHEN** the TUI refreshes rejected marker rows
- **THEN** `rejected-visible` is displayed as a rejected row from `/repo/openspec/changes`
- **AND** the row does NOT show the `NEW` badge
- **AND** no queue or selection intent is created for `rejected-visible`

### Requirement: TUI rejected row is visible but not selectable

When a change directory contains both `openspec/changes/<change_id>/proposal.md` and `openspec/changes/<change_id>/REJECTED.md`, the TUI change list SHALL display that change as a read-only `rejected` row rather than omitting it entirely.

A rejected row SHALL NOT participate in execution mark, queue, or resume controls. The TUI MUST keep its frontend-visible execution mark cleared (`selected = false`), MUST ignore queue-oriented key operations for that row, MUST NOT label the row with the `NEW` badge, and MUST visibly present the row's terminal status as `rejected` in both Select and Running mode.

Rejected row discovery during local TUI auto-refresh MUST use the same captured repository root as active change discovery. It MUST NOT depend on ambient process current working directory after TUI startup.

<!-- Expected canonical result after archive: `tui-state` will state that rejected marker row discovery is repo-root based during local TUI refresh and still never produces NEW badges or queue intent. -->

#### Scenario: Rejected change is shown in TUI list

- **GIVEN** `openspec/changes/fix-auth/proposal.md` exists
- **AND** `openspec/changes/fix-auth/REJECTED.md` exists
- **WHEN** the TUI refreshes its change list
- **THEN** `fix-auth` is displayed in the list
- **AND** its display status is `rejected`

#### Scenario: Rejected row cannot gain an execution mark

- **GIVEN** `fix-auth` is displayed as a `rejected` row in the TUI
- **WHEN** the user presses Space on that row
- **THEN** the row remains `selected = false`
- **AND** no x mark is shown for `fix-auth`
- **AND** the display status remains `rejected`

#### Scenario: Rejected row is ignored by queue-oriented actions

- **GIVEN** `fix-auth` is displayed as a `rejected` row in the TUI
- **WHEN** the user invokes queue or resume-oriented actions such as `@` or `F5`
- **THEN** `fix-auth` is not added to the execution queue
- **AND** no execution start is requested for `fix-auth`

#### Scenario: Select mode shows rejected status label

- **GIVEN** `fix-auth` is displayed as a `rejected` row in the TUI Select mode
- **WHEN** the change list row is rendered
- **THEN** the row visibly includes the label `[rejected]`
- **AND** the row does NOT show the `NEW` badge

#### Scenario: Running mode keeps rejected status label

- **GIVEN** `fix-auth` is displayed as a `rejected` row in the TUI Running mode
- **WHEN** the change list row is rendered
- **THEN** the row visibly includes the label `[rejected]`

#### Scenario: Marker removal reactivates the change as unselected active row

- **GIVEN** `fix-auth` was previously shown as a `rejected` row
- **AND** the user removes `openspec/changes/fix-auth/REJECTED.md` from the base branch
- **WHEN** the TUI refreshes after `fix-auth` reappears in the active listing
- **THEN** `fix-auth` is shown as `not queued`
- **AND** `fix-auth` remains `selected = false` until explicitly marked again

#### Scenario: Rejected row refresh uses captured repository root

- **GIVEN** local TUI mode started from repository root `/repo`
- **AND** the process current working directory later differs from `/repo`
- **AND** `/repo/openspec/changes/rejected-visible/proposal.md` exists
- **AND** `/repo/openspec/changes/rejected-visible/REJECTED.md` exists
- **WHEN** the TUI refreshes rejected marker rows
- **THEN** `rejected-visible` is displayed as a rejected row from `/repo/openspec/changes`
- **AND** the row does NOT show the `NEW` badge
- **AND** no queue or selection intent is created for `rejected-visible`

### Requirement: Scheduler dependency diagnostics are state-transition driven

The scheduler MUST emit dependency blocked/resolved diagnostics and events based on dependency blocker state transitions, not merely because a polling loop re-checked the same queued change.

For each blocked queued change, the scheduler MUST compare the current dependency blocker observation to the last emitted blocker observation for that change. The blocker observation MUST distinguish at least the blocked change id, unresolved dependency ids, and dependency target classes. Equivalent blocker observations MUST be treated as no-ops for diagnostic/event emission.

Any remembered blocker observation state MUST be in-memory and non-authoritative. It MUST NOT be persisted under `~/.local/state/cflx/**`, and it MUST NOT be used to decide scheduling eligibility, resume routing, acceptance routing, archive routing, or next-action behavior.

#### Scenario: unchanged dependency blocker emits once

- **GIVEN** change `feature-b` is queued
- **AND** change `feature-b` is blocked by dependency `feature-a`
- **AND** the scheduler has already emitted a `DependencyBlocked` diagnostic/event for the same blocker observation
- **WHEN** the scheduler loop evaluates `feature-b` again and `feature-a` has not changed dependency class or resolution state
- **THEN** no additional `DependencyBlocked` event is emitted for `feature-b`
- **AND** no additional TUI user-visible dependency blocked log is produced for that unchanged blocker observation

#### Scenario: changed dependency blocker emits again

- **GIVEN** change `feature-b` was previously blocked by dependency `feature-a`
- **WHEN** the blocker observation changes, such as the unresolved dependency set changes or `feature-a` changes from queued to rejected
- **THEN** the scheduler emits a new dependency blocked diagnostic/event for `feature-b`
- **AND** the diagnostic identifies the changed blocker state rather than silently suppressing it

#### Scenario: dependency resolution emits once per blocked transition

- **GIVEN** change `feature-b` previously emitted a dependency blocked diagnostic/event
- **WHEN** its dependencies become resolved
- **THEN** the scheduler emits one `DependencyResolved` event for `feature-b`
- **AND** later scheduler loops do not re-emit `DependencyResolved` while `feature-b` remains unblocked
- **AND** if `feature-b` becomes blocked again later, that later blocked transition can emit a new blocked diagnostic/event

#### Scenario: diagnostic suppression does not control scheduling

- **GIVEN** a dependency blocker observation has been remembered for diagnostic suppression
- **WHEN** the scheduler evaluates which changes are executable
- **THEN** executable selection is still derived from analysis, workspace state, git state, and in-flight execution state
- **AND** deleting external log/state directories such as `~/.local/state/cflx/**` does not change the next action chosen for the same workspace contents

### Requirement: TUI dependency transition logs are idempotent

TUI handling of dependency blocked and dependency resolved events MUST be idempotent for user-visible logs. A duplicate event that does not change the displayed row status MUST NOT append another identical user-visible log entry.

#### Scenario: duplicate dependency blocked event is a TUI log no-op

- **GIVEN** change `feature-b` is already displayed as `blocked` in the TUI
- **WHEN** the TUI receives another dependency blocked event for `feature-b` without a display state transition
- **THEN** the TUI keeps `feature-b` displayed as `blocked`
- **AND** the TUI does not append another identical dependency blocked log entry

#### Scenario: dependency resolved logs only on blocked-to-queued transition

- **GIVEN** change `feature-b` is displayed as `blocked` in the TUI
- **WHEN** the TUI receives a dependency resolved event for `feature-b`
- **THEN** the TUI changes the displayed status to `queued`
- **AND** the TUI appends one dependency resolved log entry
- **WHEN** the TUI receives another dependency resolved event for `feature-b` while it is no longer displayed as `blocked`
- **THEN** the TUI does not append another dependency resolved log entry

### Requirement: ChangeDequeued イベントで選択状態を解除する

TUI は `OrchestratorEvent::ChangeDequeued` を受信したとき、対象 change の `selected` フラグを `false` に設定し、`display_status_cache` を `"not queued"` に更新しなければならない（MUST）。

#### Scenario: force-kill 後に選択マークが消える

**Given**: Running モードで change `alpha` が `"applying"` 状態で `selected=true` である
**When**: `OrchestratorEvent::ChangeDequeued { change_id: "alpha" }` が TUI に届く
**Then**: `alpha` の `selected` が `false` になる
**And**: `alpha` の `display_status_cache` が `"not queued"` になる
**And**: `alpha` のチェックボックスが `[ ]`（未選択）で表示される

### Requirement: TUI execution and modal interaction state are orthogonal

The TUI MUST represent orchestration execution state independently from transient modal interaction state. Execution state MUST contain only select, running, stopping, stopped, and fatal-error lifecycle modes. QR display, worktree-delete confirmation, and single-change force-kill confirmation MUST be optional modal interactions layered over that execution state and MUST NOT replace or restore it through a captured previous-mode value.

Each destructive confirmation MUST contain its identity-bearing payload within the typed modal state. The TUI MUST NOT store independently mutable modal and confirmation payload fields whose combinations can become inconsistent. This process-local UI state MUST remain non-durable and MUST NOT become authoritative input for scheduler dispatch, resume routing, acceptance, archive, merge, or next-action selection.

#### Scenario: QR round trip preserves running execution

- **GIVEN** the TUI execution state is running
- **WHEN** the operator opens and closes the QR popup
- **THEN** the execution state remains running throughout the interaction
- **AND** closing the popup does not restore a captured copy of the execution state

#### Scenario: QR survives background execution transition

- **GIVEN** the QR popup is visible over running execution
- **WHEN** a typed execution event changes execution to stopping, stopped, or error while the Web URL remains available
- **THEN** the QR popup remains visible over the latest execution state
- **AND** closing it exposes that latest execution state

#### Scenario: QR invalidates when Web URL disappears

- **GIVEN** the QR popup is visible
- **WHEN** Web monitoring is disabled or the current Web URL is removed
- **THEN** the QR modal is cleared
- **AND** the current execution state remains unchanged

#### Scenario: worktree confirmation survives execution transition

- **GIVEN** a worktree-delete confirmation contains a path and branch identity that remains present and delete-eligible in a fresh worktree observation
- **WHEN** the underlying execution mode changes
- **THEN** the confirmation remains visible
- **AND** cancel or confirm does not restore a captured execution mode

#### Scenario: worktree refresh invalidates stale confirmation

- **GIVEN** a worktree-delete confirmation contains a path and branch identity
- **WHEN** a fresh worktree observation shows that target absent, main, active, already deleting, or bound to a different identity
- **THEN** the typed modal and its payload are cleared atomically
- **AND** a later key event cannot submit the stale delete

#### Scenario: force-kill survives Running to Stopping while target remains active

- **GIVEN** force-kill confirmation targets retryable active work in Running execution
- **WHEN** execution changes to Stopping and the target remains authoritative retryable active work
- **THEN** the force-kill confirmation remains visible
- **AND** canceling it preserves Stopping execution

#### Scenario: force-kill target transition invalidates confirmation

- **GIVEN** force-kill confirmation targets a change
- **WHEN** authoritative state shows the target terminal, dequeued, absent, non-active, non-retryable, or otherwise invalid for stop-and-dequeue
- **THEN** the typed modal and target payload are cleared atomically
- **AND** a later key event cannot submit the stale stop-and-dequeue intent

#### Scenario: confirmation revalidates authoritative state

- **GIVEN** a destructive confirmation remains visible after its target changed between display and confirmation input
- **WHEN** the operator confirms the action
- **THEN** the existing shared operator or repository-backed worktree service revalidates current identity and eligibility before mutation
- **AND** stale identity, failed cancellation, missing termination evidence, timeout, or invalid status does not mutate the invalid target

### Requirement: TUI renders execution base and modal overlay independently

The TUI MUST derive its base status, controls, elapsed-time presentation, and view content from execution and view state. It MUST render valid QR and confirmation interactions as overlays after the base presentation without changing the execution state. It MUST NOT use a fallback that rewrites unsupported or newly introduced state combinations to Select or Running.

#### Scenario: worktree confirmation overlays Error base

- **GIVEN** a still-valid worktree-delete confirmation is visible
- **AND** the underlying execution mode becomes Error
- **WHEN** the TUI renders the next frame
- **THEN** the base presentation retains Error status and retry semantics
- **AND** the worktree confirmation is rendered above it

#### Scenario: force-kill overlays Stopping base while valid

- **GIVEN** force-kill confirmation remains valid after execution enters Stopping
- **WHEN** the TUI renders the next frame
- **THEN** the base presentation retains Stopping status and controls
- **AND** the force-kill confirmation is rendered above it

#### Scenario: invalidated force-kill reveals terminal base

- **GIVEN** a force-kill confirmation is invalidated by a terminal or non-active target transition
- **WHEN** the TUI renders the next frame
- **THEN** no force-kill overlay is rendered
- **AND** the current execution base is rendered without conversion to Running

### Requirement: Bulk mark follows execution lifecycle and modal input ownership

The TUI MUST admit single-row and bulk execution-mark input for visible non-terminal rows in Select, Running, Stopping, Stopped, and Error execution modes. Execution lifecycle timing, active/retry/wait status, apply-iteration-limit evidence, and current parallel eligibility MUST NOT make a pre-archive execution mark immutable.

An execution mark is process-local next-run target intent only. Mark mutation MUST NOT mutate queue intent, stop or dequeue current work, issue cancellation, create retry or resolve intent, run hooks, wake a scheduler, or change process mode. Current-state run eligibility SHALL be evaluated at final start/retry admission instead.

The TUI MUST consume all key input while a warning or interaction modal is active and MUST NOT route `x`, Space, or any other ordinary command to the underlying view. Terminal rows (`archived`, `merged`, `pushed`, and `rejected`) MUST remain outside mark controls and Space on them MUST be a silent no-op.

#### Scenario: every execution mode admits pre-archive marks

- **GIVEN** the Changes view is active with no warning or interaction modal
- **AND** execution mode is Select, Running, Stopping, Stopped, or Error
- **AND** the target is a visible non-terminal row
- **WHEN** the operator presses Space or `x`
- **THEN** the TUI updates only process-local execution marks
- **AND** current queue, runtime, retry, resolve, cancellation, scheduler, hook, and mode state remain unchanged

#### Scenario: active and limited rows retain future intent

- **GIVEN** a visible non-terminal change is active or carries active Apply iteration-limit evidence
- **WHEN** the operator toggles its mark
- **THEN** the mark changes without stopping or retrying the current run
- **AND** final run admission remains responsible for deciding future executability

#### Scenario: overlay consumes mark input

- **GIVEN** a warning popup, QR, worktree-delete confirmation, or force-kill confirmation owns input
- **WHEN** the operator presses Space or `x`
- **THEN** the overlay handles or consumes the key according to its interaction contract
- **AND** the underlying mark action does not run

#### Scenario: terminal row ignores mark input

- **GIVEN** the cursor is on an archived, merged, pushed, or rejected row
- **WHEN** the operator presses Space
- **THEN** no execution mark or other state changes
- **AND** no warning is presented

<!-- Expected canonical result after archive: `tui-state` will keep modal input ownership but remove lifecycle-based mark immutability and treat post-archive Space as silent no-op. -->
