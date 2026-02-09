# tui-key-hints Specification

## Purpose
Defines TUI key binding hints display based on application mode.
## Requirements
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

### Requirement: Context-Aware Key Hints in Running Mode

The TUI SHALL display dynamic key hints in running mode consistent with select mode.

Changes panel title SHALL show only change-related keys.
App-level control keys (Esc, Ctrl+C) SHALL be shown in Status panel title instead of Changes panel.

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

### Requirement: 未コミット change の操作ヒントを非表示にする
並列モードで未コミットの change が選択中の場合、Changes パネルのキーヒントは選択・承認に関する操作を表示してはならない（SHALL）。

#### Scenario: 未コミット change は選択ヒントを表示しない
- **GIVEN** TUI が並列モードで表示されている
- **AND** カーソルが未コミットの change にある
- **WHEN** Changes パネルを描画する
- **THEN** "Space: queue" と "@: approve" のキーヒントは表示されない

### Requirement: 未コミット change は操作不可として表示する

未コミットの change は Changes パネルで操作不可の状態として表示しなければならない（SHALL）。

ここでいう未コミット change には、`HEAD` に存在しない change だけでなく、`openspec/changes/<change_id>/` 配下に未コミットまたは未追跡ファイルが存在する change も含まれる。

並列モードでは、未コミット change がユーザーに分かる形で表示されること。Archived 状態の行はアーカイブ済み表示を優先し、`UNCOMMITED` バッジを表示してはならない（SHALL NOT）。

#### Scenario: Archived 行には `UNCOMMITED` を表示しない

- **GIVEN** the TUI is in parallel mode
- **AND** a change row is in `Archived` status
- **WHEN** the Changes list is rendered
- **THEN** the row SHALL NOT display the `UNCOMMITED` badge
- **AND** the row SHALL display the archived checkbox styling (e.g., gray `[x]`)

#### Scenario: change 配下に未コミットがある行には `UNCOMMITED` を表示する

- **GIVEN** the TUI is in parallel mode
- **AND** a change row is eligible to be queued (e.g., `NotQueued` or `Queued`)
- **AND** the change has uncommitted or untracked files under `openspec/changes/<change_id>/`
- **WHEN** the Changes list is rendered
- **THEN** the row SHALL be grayed out
- **AND** the row SHALL NOT display a checkbox
- **AND** the row SHALL display the `UNCOMMITED` badge

### Requirement: Log Panel Toggle Hint
Changes ビューのChangesパネルはログパネルの切り替え操作として `l: logs` を表示しなければならない（SHALL）。

#### Scenario: Select mode shows log toggle hint
- **GIVEN** TUI is in select mode
- **WHEN** Changes panel is rendered
- **THEN** key hints include "l: logs"

#### Scenario: Running mode shows log toggle hint
- **GIVEN** TUI is in running mode
- **WHEN** Changes panel is rendered
- **THEN** key hints include "l: logs"

### Requirement: Active Change Stop Hint
Changes パネルは、カーソルが active change にある場合、`Space: stop` を表示しなければならない（SHALL）。

#### Scenario: Running mode shows Space: stop for active change
- **GIVEN** the TUI is in running mode
- **AND** the cursor is on a change with `queue_status.is_active() == true`
- **WHEN** the Changes panel is rendered
- **THEN** key hints include "Space: stop"
