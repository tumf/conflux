# tui-key-hints Specification

## Purpose
Defines TUI key binding hints display based on application mode.
## Requirements
### Requirement: Context-Aware Key Hints in Select Mode

`MergeWait` の change が選択中の場合、TUI は解決操作として `M` を提示しなければならない（SHALL）。

`MergeWait` 以外の change が選択中の場合、TUI は `M` 操作ヒントを表示してはならない（SHALL NOT）。

#### Scenario: `MergeWait` の行では `M` を表示する
- **GIVEN** the TUI is in select mode
- **AND** the cursor is on a change in `MergeWait` status
- **WHEN** the Changes list is rendered
- **THEN** the Changes panel key hints SHALL show "M: resolve"

### Requirement: Context-Aware Key Hints in Running Mode

The TUI SHALL display dynamic key hints in running mode consistent with select mode.

Changes panel title SHALL show only change-related keys.
App-level control keys (Esc, q) SHALL be shown in Status panel title instead of Changes panel.

#### Scenario: Running mode shows appropriate keys

- **GIVEN** the TUI is in running mode
- **WHEN** changes exist
- **THEN** the Changes panel key hints SHALL show selection keys based on current item state
- **AND** the Changes panel title SHALL NOT show "Esc: stop"
- **AND** the Changes panel title SHALL NOT show "q: quit"

#### Scenario: Running mode with empty list

- **GIVEN** the TUI is in running mode
- **WHEN** the changes list is empty
- **THEN** the Changes panel key hints SHALL NOT show selection keys
- **AND** the Changes panel title SHALL NOT show "Esc: stop"
- **AND** the Changes panel title SHALL NOT show "q: quit"

### Requirement: Approval State Transition in Select Mode

The TUI SHALL transition change state correctly when pressing `@` key in select mode.

#### Scenario: Approve unapproved change adds to queue

- **GIVEN** the TUI is in select mode
- **AND** the cursor is on an unapproved change showing `[ ]`
- **WHEN** the user presses `@`
- **THEN** the change SHALL become approved AND queued showing `[x]`
- **AND** a log message "Approved and queued: {id}" SHALL appear
- **AND** the logs panel SHALL become visible

#### Scenario: Unapprove approved change removes from queue

- **GIVEN** the TUI is in select mode
- **AND** the cursor is on an approved+queued change showing `[x]`
- **WHEN** the user presses `@`
- **THEN** the change SHALL become unapproved showing `[ ]`
- **AND** the change SHALL be removed from the queue
- **AND** a log message SHALL appear

### Requirement: Approval State Transition in Running Mode

The TUI SHALL allow approval without auto-queuing in running mode.

#### Scenario: Approve unapproved change without queuing

- **GIVEN** the TUI is in running mode
- **AND** the cursor is on an unapproved change showing `[ ]`
- **WHEN** the user presses `@`
- **THEN** the change SHALL become approved but NOT queued showing `[@]`
- **AND** a log message "Approved (not queued): {id}" SHALL appear

Runningモードで`MergeWait`のchangeが選択中の場合、`M`によるmerge resolveを開始できなければならない（SHALL）。

#### Scenario: RunningモードでMergeWaitのchangeをresolveできる
- **GIVEN** the TUI is in running mode
- **AND** the cursor is on a change in `MergeWait` status
- **AND** a resolve operation is not in progress
- **WHEN** the user presses `M`
- **THEN** the resolve command SHALL be triggered for the selected change
- **AND** the change status SHALL transition to `resolving` while the resolve runs

### Requirement: App Control Keys in Status Panel Title

停止/待機中に `MergeWait` が存在する場合でも、TUI は自動で処理を再開する操作ヒントを追加してはならない（SHALL NOT）。

#### Scenario: `MergeWait` が存在しても自動再開のヒントは増やさない
- **GIVEN** the TUI is in stopped mode
- **AND** at least one change is in `MergeWait`
- **WHEN** the Status panel title is rendered
- **THEN** the title SHALL NOT imply automatic resume of merge

### Requirement: Approval State Transition in Stopped Mode

停止モードで `MergeWait` の change が選択中の場合、`M` は選択中 change のみを解決する単発操作でなければならない（SHALL）。

さらに、`M` による merge resolve の実行中、TUI は対象 change のステータス表示を `resolving` として示し、TUI の描画ループをブロックしてはならない（SHALL NOT）。

resolve実行中は `M` による追加操作を受け付けてはならない（SHALL NOT）。

#### Scenario: resolve中は `M` を受け付けない
- **GIVEN** the TUI is in stopped mode
- **AND** the cursor is on a change in `MergeWait`
- **AND** a resolve operation is in progress
- **WHEN** the user presses `M`
- **THEN** the resolve command SHALL NOT be triggered
- **AND** the user SHALL receive a warning message

### Requirement: 未コミット change の操作ヒントを非表示にする
並列モードで未コミットの change が選択中の場合、Changes パネルのキーヒントは選択・承認に関する操作を表示してはならない（SHALL）。

#### Scenario: 未コミット change は選択ヒントを表示しない
- **GIVEN** TUI が並列モードで表示されている
- **AND** カーソルが未コミットの change にある
- **WHEN** Changes パネルを描画する
- **THEN** "Space: queue" と "@: approve" のキーヒントは表示されない

### Requirement: 未コミット change は操作不可として表示する

未コミットの change は Changes パネルで操作不可の状態として表示しなければならない（SHALL）。

並列モードでは、未コミット change がユーザーに分かる形で表示されること。Archived 状態の行はアーカイブ済み表示を優先し、`UNCOMMITED` バッジを表示してはならない（SHALL NOT）。

#### Scenario: Archived 行には `UNCOMMITED` を表示しない

- **GIVEN** the TUI is in parallel mode
- **AND** a change row is in `Archived` status
- **WHEN** the Changes list is rendered
- **THEN** the row SHALL NOT display the `UNCOMMITED` badge
- **AND** the row SHALL display the archived checkbox styling (e.g., gray `[x]`)

#### Scenario: 未コミットかつキュー可能な行には `UNCOMMITED` を表示する

- **GIVEN** the TUI is in parallel mode
- **AND** a change row is eligible to be queued (e.g., `NotQueued` or `Queued`)
- **AND** the change is not parallel-eligible (uncommitted)
- **WHEN** the Changes list is rendered
- **THEN** the row SHALL be grayed out
- **AND** the row SHALL NOT display a checkbox
- **AND** the row SHALL display the `UNCOMMITED` badge

