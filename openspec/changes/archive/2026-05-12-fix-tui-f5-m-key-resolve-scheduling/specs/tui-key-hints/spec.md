## MODIFIED Requirements

### Requirement: Context-Aware Key Hints in Select Mode

`MergeWait` の change が選択中の場合、TUI は解決操作として `M` を提示しなければならない（SHALL）。

resolve 実行中は `M: queue resolve` を表示し、resolve 未実行中は `M: resolve` を表示しなければならない（SHALL）。

`MergeWait` 以外の change が選択中の場合、TUI は `M` 操作ヒントを表示してはならない（SHALL NOT）。

`F5` hints SHALL describe app-level orchestration start/resume/retry only and MUST NOT imply cursor-local `MergeWait` resolve behavior.

<!-- Expected canonical result after archive: `tui-key-hints` will keep M as the only cursor-local MergeWait resolve hint and prevent F5 hints from implying resolve behavior. -->

#### Scenario: `MergeWait` の行では `M: resolve` を表示する

- **GIVEN** the TUI is in select mode
- **AND** the cursor is on a change in `MergeWait` status
- **AND** a resolve operation is not in progress
- **WHEN** the Changes list is rendered
- **THEN** the Changes panel key hints SHALL show `M: resolve`
- **AND** any `F5` hint SHALL remain an orchestration run/resume/retry hint, not a merge resolve hint

#### Scenario: resolve 実行中は `M: queue resolve` を表示する

- **GIVEN** the TUI is in select mode
- **AND** the cursor is on a change in `MergeWait` status
- **AND** a resolve operation is in progress
- **WHEN** the Changes list is rendered
- **THEN** the Changes panel key hints SHALL show `M: queue resolve`
- **AND** any `F5` hint SHALL remain independent of the cursor row

#### Scenario: `MergeWait` 以外の行では `M` を表示しない

- **GIVEN** the TUI is in select mode
- **AND** the cursor is on a change not in `MergeWait` status
- **WHEN** the Changes list is rendered
- **THEN** the Changes panel key hints SHALL NOT show `M: resolve`
- **AND** the absence of an M hint SHALL NOT affect F5 orchestration availability for marked runnable work

### Requirement: Approval State Transition in Stopped Mode

停止モードで `MergeWait` の change が選択中の場合、`M` は選択中 change のみを対象として scheduler-owned resolve retry intent を登録しなければならない（SHALL）。

`M` 押下直後の表示は、scheduler-owned retry intent が受理されている間 `resolve pending` であってよい（MAY）。実際の merge/resolve が開始された後にのみ、対象 change は scheduler event によって `resolving` として表示されなければならない（SHALL）。

resolve 実行中に `M` が押された場合、対象 change は `ResolveWait` として待ち行列へ追加されなければならない（SHALL）。

<!-- Expected canonical result after archive: `tui-key-hints` will no longer imply that M immediately displays `resolving`; M registers intent, then scheduler events drive `resolving`. -->

#### Scenario: Stopped mode M registers resolve intent

- **GIVEN** the TUI is in stopped mode
- **AND** the cursor is on a change in `MergeWait`
- **AND** a resolve operation is not in progress
- **WHEN** the user presses `M`
- **THEN** scheduler-visible resolve retry intent SHALL be registered for the selected change
- **AND** the row MAY display `resolve pending` while the scheduler evaluates and starts the retry
- **AND** the row SHALL display `resolving` only after scheduler-owned resolve execution starts

#### Scenario: resolve 実行中の `M` は待ち行列へ追加する

- **GIVEN** the TUI is in stopped mode
- **AND** the cursor is on a change in `MergeWait`
- **AND** a resolve operation is in progress
- **WHEN** the user presses `M`
- **THEN** the change status SHALL transition to `ResolveWait`
- **AND** the resolve command SHALL NOT be triggered immediately as a second concurrent resolve
