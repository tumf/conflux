## MODIFIED Requirements

### Requirement: auto-resumable-merge-deferred-triggers-resolve

TUI は `MergeDeferred(auto_resumable=true)` イベントを受信し、かつ同一 Project 内で resolve が実行中でない場合、Change を `ResolveWait` に遷移させた上で scheduler-owned resolve retry intent を開始または通知しなければならない（MUST）。`auto_resumable=true` は resolve カウンターまたは reducer が観測する base-mutating lane occupancy による判定結果のみから設定されなければならず（MUST）、dirty reason の文字列解析には依存してはならない（MUST NOT）。

TUI startup reconciliation MUST derive manual merge-wait eligibility from workspace-local archived-but-not-yet-merged evidence. When `ChangesRefreshed.merge_wait_ids` identifies a change whose reducer entry is idle, not queued, non-terminal, and has no stronger wait state, the shared reducer MUST restore that change as `MergeWait` before manual resolve admission is evaluated. The same reconciliation MUST NOT demote active, pending, queued, terminal, rejected, or error state to `MergeWait`.

Manual resolve intent is reducer-owned scheduler work. When a user starts resolve from a `MergeWait` row with `M`, including a row reconstructed during TUI startup, any visible `resolve pending` state MUST correspond to reducer-owned retry membership that the scheduler can consume. Accepted intent MUST start or notify scheduler-owned retry evaluation. A change without archived-but-not-yet-merged workspace evidence MUST NOT become resolve-eligible solely because a stale frontend cache or arbitrary command names it.

<!-- Expected canonical result after archive: `tui-resolve` will require workspace-derived startup merge-wait evidence to restore reducer-owned manual resolve eligibility without weakening admission for ordinary not-queued changes. -->

#### Scenario: startup workspace evidence restores manual merge wait

**Given**: the TUI starts with a fresh reducer entry for change `alpha`
**And**: workspace and Git evidence identify `alpha` as archived but not yet merged into base
**And**: `alpha` is idle, not queued, non-terminal, and has no stronger wait state
**When**: the refresh loop publishes `ChangesRefreshed` with `alpha` in `merge_wait_ids`
**Then**: the shared reducer reports `alpha` as `merge wait`
**And**: the TUI row is synchronized from that reducer-owned status

#### Scenario: M on startup-restored merge wait starts scheduler-owned retry

**Given**: startup reconciliation restored change `alpha` as reducer-owned `merge wait`
**And**: no scheduler is currently running
**When**: the user presses `M` on `alpha`
**Then**: the reducer records scheduler-consumable `ResolveWait` for `alpha`
**And**: the resolve reservation becomes active
**And**: the scheduler starts manual retry evaluation for `alpha`
**And**: the TUI does not report ordinary zero-change success while the retry intent remains pending

#### Scenario: startup refresh preserves stronger reducer state

**Given**: change `alpha` is resolving, resolve pending, rejecting, reject pending, queued, merged, rejected, or in error
**And**: a refresh also reports `alpha` in `merge_wait_ids`
**When**: the shared reducer reconciles the workspace observation
**Then**: `alpha` retains its stronger reducer-owned state
**And**: the refresh does not create a duplicate manual resolve reservation

#### Scenario: arbitrary not-queued target remains ineligible

**Given**: change `alpha` is idle and not queued
**And**: no workspace evidence identifies `alpha` as archived but not yet merged into base
**When**: a TUI or `/api/v2` caller submits manual resolve intent for `alpha`
**Then**: the shared run-control service rejects the target as ineligible
**And**: no resolve reservation or scheduler dispatch is created
