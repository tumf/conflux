### Requirement: イベント送信はヘルパー関数経由で行う
オーケストレーションループからの `ExecutionEvent` 送信は `dispatch_event()` ヘルパー関数を経由しなければならない（MUST）。個別に `tx.send()`, `shared_state.write().await.apply_execution_event()`, `ws.apply_execution_event()` を直接呼び出すパターンを使ってはならない（SHALL NOT）。

#### Scenario: TUI オーケストレータがヘルパー経由でイベントを送信する
- **WHEN** `tui/orchestrator.rs` の `run_orchestrator` 内でイベントを送信する
- **THEN** `dispatch_event()` ヘルパーが呼ばれる
- **AND** ヘルパー内で Reducer (shared_state), TUI channel (tx), Web (web_state) の3箇所に送信される

#### Scenario: CLI オーケストレータも同じヘルパーを使う
- **WHEN** `orchestrator.rs` 内で Web 向けにイベントを送信する
- **THEN** 同様のヘルパー関数が使われる
- **AND** 直接 `apply_execution_event()` を呼び出さない

### Requirement: Execution events identify their target change explicitly

Execution events that mutate per-change runtime activity SHALL identify the target change explicitly in the event payload. Events used to synchronize workspace activity from parallel execution MUST be applicable without consulting unrelated orchestrator-global cursors such as `current_change_id`.

#### Scenario: Workspace status update names its target change

- **GIVEN** a parallel workspace transitions into `Rejecting`
- **WHEN** the runtime emits a workspace status synchronization event
- **THEN** the event payload includes the target change identifier
- **AND** downstream reducers can update the matching runtime entry directly

### Requirement: Archive retry and resume events identify retry cause explicitly

Execution events used to synchronize archive retry or archive resume activity SHALL identify the target change and the archive retry/resume cause explicitly.

#### Scenario: archive retry event includes target change and reason

- **GIVEN** a parallel workspace for change `delta` schedules another archive attempt
- **WHEN** the runtime emits the archive retry synchronization event
- **THEN** the event payload includes the target change identifier
- **AND** the payload includes the archive primary reason and summary
- **AND** downstream reducers or UI layers can render why archive is looping without consulting unrelated global cursors or parsing free-form log text only

### Requirement: Apply commit presentation MUST use an explicit ephemeral event

Conflux MUST expose final Apply commit presentation through an execution event that explicitly identifies the target change, commit phase, and attempt. Reducers MAY retain the phase in process memory for TUI and additive API presentation, but the canonical lifecycle MUST remain `applying`. Commit presentation MUST NOT be persisted or used for scheduler eligibility, resume routing, acceptance, archive, merge, or next-action decisions.

#### Scenario: Finalization changes TUI presentation to commit

**Given**: a change is in the canonical Applying activity
**When**: finalization starts stage checking and the verified commit sequence
**Then**: an explicit commit-phase event identifies the change and attempt
**And**: the TUI renders `[commit]` without changing the canonical `applying` status

#### Scenario: Repair iteration restores apply presentation

**Given**: a final commit hook or stage cleanliness gate requires Apply repair
**When**: the next Apply iteration starts
**Then**: commit presentation is cleared
**And**: the TUI renders `[apply]`

#### Scenario: Completion and failure do not leave stale commit presentation

**Given**: commit presentation is active
**When**: finalization completes, fails, or is cancelled
**Then**: the reducer clears commit presentation
**And**: subsequent rendering does not retain stale `[commit]` state

#### Scenario: Restart ignores commit presentation

**Given**: a process stops while commit presentation is active
**When**: Conflux starts again
**Then**: routing is derived from workspace files and Git state
**And**: absence of the prior process-local commit phase does not change the next action
