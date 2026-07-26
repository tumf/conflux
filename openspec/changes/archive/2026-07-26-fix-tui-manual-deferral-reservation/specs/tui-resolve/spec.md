## MODIFIED Requirements

### Requirement: auto-resumable-merge-deferred-triggers-resolve

TUI は `MergeDeferred(auto_resumable=true)` イベントを受信し、かつ同一 Project 内で resolve が実行中でない場合、Change を `ResolveWait` に遷移させた上で scheduler-owned resolve retry intent を開始または通知しなければならない（MUST）。`auto_resumable=true` は resolve カウンターまたは reducer が観測する base-mutating lane occupancy による判定結果のみから設定されなければならず（MUST）、dirty reason の文字列解析には依存してはならない（MUST NOT）。

`is_resolving` は Project スコープの resolve 直列化フラグであり、同一 Project 内で resolve 操作が同時に 1 つしか実行されないことを保証する。このフラグは resolve 操作同士の直列化のみに使用し、apply/accept/archive パイプラインの開始・再開・リトライをブロックしてはならない（MUST NOT）。

Manual resolve intent is reducer-owned scheduler work. When a user starts resolve from a `MergeWait` row with `M`, any visible `resolve pending` state MUST correspond to reducer-owned retry membership that the scheduler can consume. The TUI MUST NOT leave a row at `resolve pending` solely because a local display transition occurred while the reducer rejected or dropped the same retry intent. Conversely, after the reducer accepts manual resolve intent, refresh-derived `merge_wait_ids` MUST NOT revert the visible row from `resolve pending` to `merge wait` while the scheduler-owned retry remains pending.

Manual retry classification MUST evaluate active resolve/base-mutating lane occupancy before dirty workspace/base evidence. Dirty state observed while another resolve/base-mutating operation is active SHALL keep the retry auto-resumable (`resolve pending`). Dirty state observed when no resolve/base-mutating operation is active SHALL become manual `merge wait` and clear scheduler-owned `ResolveWait` membership until explicit retry intent is accepted again.

When `MergeDeferred(auto_resumable=false)` is emitted for a manual retry before `ResolveStarted`, the TUI MUST treat the event as closing any optimistic local resolve reservation for that change. It MUST remove the change from TUI-local resolve queue membership, MUST preserve the reducer-derived `merge wait` display, and MUST NOT recreate `resolve pending` or another resolve command solely because the local serialization flag was reserved by the preceding `M`. If a different change is actually resolving, that different change's serialization ownership MUST remain active.

When a scheduler is already running because other changes are applying, accepting, or archiving, pressing `M` on a `MergeWait` row MUST notify the existing scheduler only after reducer-owned retry intent is accepted. The row may display `resolve pending` while waiting on scheduler/base-lane capacity, but it MUST eventually transition through scheduler events to `resolving` / `merged` or back to `merge wait` with visible failure/defer evidence.

When no scheduler is running, pressing `M` on a `MergeWait` row MAY start a scheduler with zero normal queued changes, but that run MUST be classified as a manual retry run rather than ordinary empty execution. The TUI MUST NOT report `Execution completed (0 changes processed)`, `All parallel changes completed`, or equivalent success while the shared reducer still contains accepted retry intent for that row. The row MUST leave `resolve pending` through scheduler-owned events or visible retry-prerequisite failure evidence.

After a dirty workspace/base manual retry returns a row to `merge wait`, pressing `M` again after the user makes retry preconditions clean MUST create or preserve scheduler-consumable retry intent for that row and MUST start or wake scheduler-owned retry evaluation. This second retry MUST NOT be suppressed solely because the same change was previously dispatched, dirty-deferred, or tracked in executor-local retry/dirty snapshots.

When a manual merge retry starts through the resolve lifecycle and the successful repository integration is reported as `MergeCompleted` rather than `ResolveCompleted`, the TUI MUST treat that `MergeCompleted` event as closing the local resolve lifecycle. It MUST clear any stale `is_resolving` reservation and MUST dispatch the next queued resolve retry intent, if one exists.

<!-- Expected canonical result after archive: `tui-resolve` will require pre-ResolveStarted manual deferral to clear the optimistic TUI reservation and preserve actionable `merge wait` without a TUI restart. -->

#### Scenario: M during active resolve remains resolve pending despite dirty evidence

**Given**: change `alpha` is currently resolving or owns the base-mutating lane
**And**: the base/workspace appears dirty because of `alpha`
**And**: change `beta` is visible as `merge wait`
**When**: the user presses `M` on `beta`
**Then**: the reducer records scheduler-consumable `ResolveWait` for `beta`
**And**: `beta` remains visible as `resolve pending` while waiting for the active resolve/base-mutating lane to clear
**And**: `beta` is not demoted to manual `merge wait` solely because the workspace/base appears dirty during `alpha`

#### Scenario: M with dirty state and no active resolve returns to merge wait

**Given**: no resolve/base-mutating operation is active
**And**: change `alpha` is visible as `merge wait`
**And**: base/workspace state is dirty or manually blocked
**When**: the user presses `M` on `alpha`
**Then**: the row may transition briefly to `resolve pending` while retry intent is evaluated
**And**: after scheduler classification, `alpha` is visible as `merge wait`
**And**: scheduler-owned `ResolveWait(alpha)` is cleared
**And**: no `ResolveStarted(alpha)` event is emitted

#### Scenario: manual deferral before ResolveStarted clears local reservation

**Given**: change `alpha` is visible as `merge wait` and no other change is actually resolving
**And**: pressing `M` has optimistically reserved the TUI-local resolve slot for `alpha`
**And**: scheduler retry evaluation emits `MergeDeferred(alpha, auto_resumable=false)` before `ResolveStarted(alpha)`
**When**: the TUI applies the reducer result and handles the deferred event
**Then**: `alpha` is visible as `merge wait`
**And**: the optimistic local resolve reservation for `alpha` is cleared
**And**: `alpha` is absent from the TUI-local resolve queue
**And**: the TUI does not emit another resolve command for `alpha`

#### Scenario: manual deferral preserves a different active resolve

**Given**: change `alpha` is actually displayed as `resolving`
**And**: change `beta` receives `MergeDeferred(beta, auto_resumable=false)`
**When**: the TUI handles the manual deferral for `beta`
**Then**: `beta` is visible as `merge wait` and is absent from local resolve queue membership
**And**: the project resolve serialization remains active for `alpha`

#### Scenario: M after dirty blocker is cleaned starts retry

**Given**: no resolve/base-mutating operation is active
**And**: change `alpha` is visible as `merge wait` because an earlier manual retry was deferred by dirty workspace/base state
**And**: scheduler-owned `ResolveWait(alpha)` and the TUI-local reservation were cleared by that dirty manual deferral
**And**: the user has cleaned the workspace/base retry preconditions
**When**: the user presses `M` on `alpha` again
**Then**: the reducer records scheduler-consumable `ResolveWait` for `alpha`
**And**: the TUI starts a manual retry scheduler run or notifies the existing scheduler
**And**: scheduler-owned retry evaluation reaches the merge attempt path for `alpha`
**And**: the retry does not require a TUI restart, another queued change, or another `M` keypress to make progress

#### Scenario: M with clean state and no active resolve starts scheduler-owned retry

**Given**: no resolve/base-mutating operation is active
**And**: change `alpha` is visible as `merge wait`
**And**: base/workspace retry preconditions are clean
**When**: the user presses `M` on `alpha`
**Then**: the reducer records scheduler-consumable `ResolveWait` for `alpha`
**And**: the scheduler starts one resolve retry for `alpha`
**And**: `alpha` transitions from `resolve pending` to `resolving` via scheduler event

#### Scenario: restarted M does not complete as zero-change success

**Given**: no scheduler is currently running
**And**: change `alpha` is visible as `merge wait`
**When**: the user presses `M` on `alpha`
**Then**: the TUI starts or schedules manual retry work
**And**: the TUI does not log successful zero-change completion while `ResolveWait(alpha)` remains in shared reducer state
**And**: `alpha` eventually becomes `resolving`, `merged`, `merge wait` with visible reason, or explicit error/stalled state
