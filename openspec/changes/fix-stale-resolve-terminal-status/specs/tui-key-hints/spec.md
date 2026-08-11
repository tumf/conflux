## MODIFIED Requirements

### Requirement: Approval State Transition in Stopped Mode

停止モードで `MergeWait` の change が選択中の場合、`M` は選択中 change のみを対象として scheduler-owned resolve retry intent を登録しなければならない（SHALL）。

`M` 押下直後の表示は、scheduler-owned retry intent が受理されている間 `resolve pending` であってよい（MAY）。実際の merge/resolve が開始された後にのみ、対象 change は scheduler event によって `resolving` として表示されなければならない（SHALL）。

resolve 実行中に `M` が押された場合、対象 change は `ResolveWait` として待ち行列へ追加されなければならない（SHALL）。

Scheduler が deferred retry を stale と分類する場合、base-branch tree comparison が対象 change の統合を証明したときは shared reducer を terminal `merged` に遷移させてから scheduler-owned retry intent を解放しなければならない（SHALL）。統合が証明されない、または証拠を安全に読み取れない場合、対象 change は manual `merge wait` を維持しなければならず（SHALL）、`ResolveWait` の消去によって `not queued` を露出してはならない（MUST NOT）。

いずれの stale retry settlement も、dirty index、tracked changes、または non-ignored untracked content を stage、commit、stash、reset、discard してはならない（MUST NOT）。

<!-- Expected canonical result after archive: `tui-key-hints` will require stale manual resolve retries to settle as repository-proven `merged` or retryable `merge wait`, never idle `not queued`, while preserving dirty content. -->

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

#### Scenario: Stale retry with proven base integration becomes merged

- **GIVEN** a change moved from `MergeWait` to `ResolveWait` after explicit `M`
- **AND** base-branch tree comparison proves the archived change is already integrated
- **WHEN** the scheduler classifies the deferred retry as stale
- **THEN** the shared reducer SHALL transition the change to terminal `merged`
- **AND** TUI and Web projections SHALL display `merged`
- **AND** the row SHALL NOT display `not queued`
- **AND** scheduler retry and base-lane ownership SHALL be released after reducer settlement

#### Scenario: Stale retry without safe completion evidence remains retryable

- **GIVEN** a change moved from `MergeWait` to `ResolveWait` after explicit `M`
- **WHEN** base integration is absent or its evidence cannot be read safely
- **THEN** the shared reducer SHALL retain or restore manual `MergeWait`
- **AND** the row SHALL NOT display `merged` or `not queued`
- **AND** the operator MAY explicitly retry after repository state is corrected

#### Scenario: Dirty content is preserved during stale settlement

- **GIVEN** the repository index or worktree contains unrelated dirty content
- **AND** a deferred resolve retry reaches stale settlement
- **WHEN** the scheduler evaluates base integration and settles reducer state
- **THEN** the dirty content and index state SHALL remain unchanged
- **AND** Conflux SHALL NOT stage, commit, stash, reset, or discard that content
