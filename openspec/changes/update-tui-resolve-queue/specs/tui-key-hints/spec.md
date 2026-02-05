## MODIFIED Requirements
### Requirement: Context-Aware Key Hints in Select Mode

`MergeWait` の change が選択中の場合、TUI は解決操作として `M` を提示しなければならない（SHALL）。

resolve 実行中は `M: queue resolve` を表示し、resolve 未実行中は `M: resolve` を表示しなければならない（SHALL）。

`MergeWait` 以外の change が選択中の場合、TUI は `M` 操作ヒントを表示してはならない（SHALL NOT）。

#### Scenario: `MergeWait` の行では `M: resolve` を表示する
- **GIVEN** the TUI is in select mode
- **AND** the cursor is on a change in `MergeWait` status
- **AND** a resolve operation is not in progress
- **WHEN** the Changes list is rendered
- **THEN** the Changes panel key hints SHALL show "M: resolve"

#### Scenario: resolve 実行中は `M: queue resolve` を表示する
- **GIVEN** the TUI is in select mode
- **AND** the cursor is on a change in `MergeWait` status
- **AND** a resolve operation is in progress
- **WHEN** the Changes list is rendered
- **THEN** the Changes panel key hints SHALL show "M: queue resolve"

#### Scenario: `MergeWait` 以外の行では `M` を表示しない
- **GIVEN** the TUI is in select mode
- **AND** the cursor is on a change not in `MergeWait` status
- **WHEN** the Changes list is rendered
- **THEN** the Changes panel key hints SHALL NOT show "M: resolve"

### Requirement: Approval State Transition in Running Mode

The TUI SHALL allow approval without auto-queuing in running mode.

#### Scenario: Approve unapproved change without queuing

- **GIVEN** the TUI is in running mode
- **AND** the cursor is on an unapproved change showing `[ ]`
- **WHEN** the user presses `@`
- **THEN** the change SHALL become approved but NOT queued showing `[@]`
- **AND** a log message "Approved (not queued): {id}" SHALL appear

Runningモードで`MergeWait`のchangeが選択中の場合、`M`によるmerge resolveを開始できなければならない（SHALL）。

resolve 実行中の場合は、`M` により対象 change を `ResolveWait` に遷移し、resolve 待ちとしてキューに追加しなければならない（SHALL）。

#### Scenario: RunningモードでMergeWaitのchangeをresolveできる
- **GIVEN** the TUI is in running mode
- **AND** the cursor is on a change in `MergeWait` status
- **AND** a resolve operation is not in progress
- **WHEN** the user presses `M`
- **THEN** the resolve command SHALL be triggered for the selected change
- **AND** the change status SHALL transition to `resolving` while the resolve runs

#### Scenario: resolve 実行中の `M` は待ち行列へ追加する
- **GIVEN** the TUI is in running mode
- **AND** the cursor is on a change in `MergeWait` status
- **AND** a resolve operation is in progress
- **WHEN** the user presses `M`
- **THEN** the change status SHALL transition to `ResolveWait`
- **AND** the resolve command SHALL NOT be triggered immediately

### Requirement: Approval State Transition in Stopped Mode

停止モードで `MergeWait` の change が選択中の場合、`M` は選択中 change のみを解決する単発操作でなければならない（SHALL）。

さらに、`M` による merge resolve の実行中、TUI は対象 change のステータス表示を `resolving` として示し、TUI の描画ループをブロックしてはならない（SHALL NOT）。

resolve 実行中に `M` が押された場合、対象 change は `ResolveWait` として待ち行列へ追加されなければならない（SHALL）。

#### Scenario: StoppedモードでMergeWaitのchangeをresolveできる
- **GIVEN** the TUI is in stopped mode
- **AND** the cursor is on a change in `MergeWait`
- **AND** a resolve operation is not in progress
- **WHEN** the user presses `M`
- **THEN** the resolve command SHALL be triggered for the selected change
- **AND** the change status SHALL transition to `resolving`

#### Scenario: resolve 実行中の `M` は待ち行列へ追加する
- **GIVEN** the TUI is in stopped mode
- **AND** the cursor is on a change in `MergeWait`
- **AND** a resolve operation is in progress
- **WHEN** the user presses `M`
- **THEN** the change status SHALL transition to `ResolveWait`
- **AND** the resolve command SHALL NOT be triggered immediately
