# tui-key-hints Specification

## Purpose
Defines TUI key binding hints display based on application mode.
## Requirements

### Requirement: Context-Aware Key Hints in Select Mode

`MergeWait` の change が選択中の場合、TUI は解決操作として `M` を提示しなければならない（SHALL）。

resolve 実行中は `M: queue resolve` を表示し、resolve 未実行中は `M: resolve` を表示しなければならない（SHALL）。

`MergeWait` 以外の change が選択中の場合、TUI は `M` 操作ヒントを表示してはならない（SHALL NOT）。

Configured start key hints SHALL describe app-level orchestration start/resume/retry only and MUST NOT imply cursor-local `MergeWait` resolve behavior. When no TUI config override exists, the configured start key label is `F5/!`.

<!-- Expected canonical result after archive: `tui-key-hints` will describe start-control hints in terms of the resolved TUI start key label rather than hardcoded F5, while keeping M as the only cursor-local MergeWait resolve hint. -->

#### Scenario: `MergeWait` の行では `M: resolve` を表示する

- **GIVEN** the TUI is in select mode
- **AND** the cursor is on a change in `MergeWait` status
- **AND** a resolve operation is not in progress
- **WHEN** the Changes list is rendered
- **THEN** the Changes panel key hints SHALL show `M: resolve`
- **AND** any configured start key hint SHALL remain an orchestration run/resume/retry hint, not a merge resolve hint

#### Scenario: resolve 実行中は `M: queue resolve` を表示する

- **GIVEN** the TUI is in select mode
- **AND** the cursor is on a change in `MergeWait` status
- **AND** a resolve operation is in progress
- **WHEN** the Changes list is rendered
- **THEN** the Changes panel key hints SHALL show `M: queue resolve`
- **AND** any configured start key hint SHALL remain independent of the cursor row

#### Scenario: `MergeWait` 以外の行では `M` を表示しない

- **GIVEN** the TUI is in select mode
- **AND** the cursor is on a change not in `MergeWait` status
- **WHEN** the Changes list is rendered
- **THEN** the Changes panel key hints SHALL NOT show `M: resolve`
- **AND** the absence of an M hint SHALL NOT affect configured start-control availability for marked runnable work

#### Scenario: Configured start key label is rendered

- **GIVEN** the resolved TUI start keybindings are `F5` and `!`
- **AND** marked runnable work exists
- **WHEN** the Changes panel key hints are rendered
- **THEN** the start-control hint SHALL use the label `F5/!`
- **AND** the hint SHALL describe app-level run/resume/retry behavior

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

The TUI SHALL ignore `@` key presses and SHALL NOT change any selection or queue state.

#### Scenario: @ key does nothing in select mode
- **GIVEN** the TUI is in select mode
- **WHEN** the user presses `@`
- **THEN** the change state remains unchanged
- **AND** no approval-related log message is shown

### Requirement: Approval State Transition in Running Mode

The TUI SHALL ignore `@` key presses and SHALL NOT change any selection or queue state.

#### Scenario: @ key does nothing in running mode
- **GIVEN** the TUI is in running mode
- **WHEN** the user presses `@`
- **THEN** the change state remains unchanged

### Requirement: App Control Keys in Status Panel Title

停止/待機中に `MergeWait` が存在する場合でも、TUI は自動で処理を再開する操作ヒントを追加してはならない（SHALL NOT）。

Status panel app-control hints SHALL use the resolved TUI start key label instead of hardcoded `F5` text.

<!-- Expected canonical result after archive: `tui-key-hints` requires status panel titles to render resolved start key labels such as `F5/!` or a configured override. -->

#### Scenario: `MergeWait` が存在しても自動再開のヒントは増やさない

- **GIVEN** the TUI is in stopped mode
- **AND** at least one change is in `MergeWait`
- **WHEN** the Status panel title is rendered
- **THEN** the title SHALL NOT imply automatic resume of merge

#### Scenario: Stopped mode status title uses configured start label

- **GIVEN** the resolved TUI start keybindings are `F5` and `!`
- **AND** the TUI is in stopped mode
- **WHEN** the Status panel title is rendered
- **THEN** the title SHALL include `F5/!: resume`

#### Scenario: Stopping mode status title uses configured start label

- **GIVEN** the resolved TUI start keybindings are `F5` and `!`
- **AND** the TUI is in stopping mode
- **WHEN** the Status panel title is rendered
- **THEN** the title SHALL include `F5/!: continue`

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

### Requirement: 未コミット change の操作ヒントを非表示にする

並列モードで `openspec/changes/<change_id>/` 配下に未コミットまたは未追跡ファイルがある change が選択中の場合、Changes パネルのキーヒントは選択に関する操作を表示してはならない（SHALL）。

`HEAD` に proposal directory が存在しないことだけを未コミット状態として表示してはならない（SHALL NOT）。ただし、proposal absence による既存の parallel queue admission rule は維持しなければならない（SHALL）。

<!-- Expected canonical result after archive: `tui-key-hints` will distinguish dirty proposal content from proposal absence when deciding whether to describe a row as uncommitted, without weakening parallel admission. -->

#### Scenario: 未コミット proposal は選択ヒントを表示しない

- **GIVEN** TUI が並列モードで表示されている
- **AND** カーソル位置の change の proposal directory に未コミットまたは未追跡ファイルがある
- **WHEN** Changes パネルを描画する
- **THEN** `Space: queue` のキーヒントは表示されない

#### Scenario: proposal absence は未コミット理由として扱わない

- **GIVEN** TUI が並列モードで表示されている
- **AND** カーソル位置の change の proposal directory は `HEAD` に存在しない
- **AND** その change の proposal directory に未コミットまたは未追跡ファイルはない
- **WHEN** Changes パネルを描画する
- **THEN** TUI はその行を未コミットであるとは表示しない
- **AND** the row remains grayed out, non-markable, and without queue affordances
- **AND** proposal absence による parallel queue admission refusal は維持される
- **AND** any refusal message identifies proposal absence from `HEAD` instead of instructing the operator to commit nonexistent dirty content

### Requirement: 未コミット change は操作不可として表示する

`openspec/changes/<change_id>/` 配下に未コミットまたは未追跡ファイルがある active change は、Changes パネルで操作不可の状態として表示しなければならない（SHALL）。

並列実行への不適格理由は区別されなければならない（SHALL）。proposal directory が `HEAD` に存在しないことだけを、未コミットまたは未追跡ファイルの存在として表示してはならない（SHALL NOT）。

実際に未コミットまたは未追跡の proposal ファイルが存在する場合のみ、TUI は正しい綴りの `UNCOMMITTED` バッジを表示しなければならない（SHALL）。Archived 状態、failed merge 状態、または retained managed worktree の存在だけを理由に `UNCOMMITTED` を表示してはならない（SHALL NOT）。managed worktree が存在する場合の `WT` 表示は独立して維持されなければならない（SHALL）。

<!-- Expected canonical result after archive: `tui-key-hints` will reserve `UNCOMMITTED` for observed dirty proposal files, retain independent `WT` display, and stop equating all parallel-ineligible reasons with dirty Git state. -->

#### Scenario: dirty active proposal は `UNCOMMITTED` を表示する

- **GIVEN** the TUI is in parallel mode
- **AND** a change row is in a queueable active status such as `NotQueued` or `Queued`
- **AND** the change has uncommitted or untracked files under `openspec/changes/<change_id>/`
- **WHEN** the Changes list is rendered
- **THEN** the row SHALL be grayed out and non-actionable
- **AND** the row SHALL display `UNCOMMITTED`

#### Scenario: clean proposal absence は `UNCOMMITTED` を表示しない

- **GIVEN** the TUI is in parallel mode
- **AND** a change is parallel-ineligible because its proposal directory is absent from `HEAD`
- **AND** no uncommitted or untracked proposal files are observed for that change
- **WHEN** the Changes list is rendered
- **THEN** the row SHALL NOT display `UNCOMMITTED`
- **AND** the row SHALL remain grayed out, non-markable, and without queue affordances
- **AND** the change SHALL remain ineligible for parallel queue admission

#### Scenario: clean proposal absence has a truthful refusal reason

- **GIVEN** a parallel-ineligible change has no proposal directory in `HEAD`
- **AND** no uncommitted or untracked proposal files are observed for that change
- **WHEN** a single-row or bulk-toggle operation reports why the change was excluded
- **THEN** the reason SHALL identify that the change is not present in `HEAD`
- **AND** the reason SHALL NOT describe the change as uncommitted or instruct the operator to commit it

#### Scenario: retained worktree marker is independent of dirty state

- **GIVEN** an archived or failed-merge change retains a clean managed worktree
- **AND** its active proposal directory is absent from `HEAD`
- **AND** the change row is displayed in a queueable status such as `NotQueued` or `Queued`
- **WHEN** the Changes list is rendered
- **THEN** the row MAY display `WT`
- **AND** the row SHALL NOT display `UNCOMMITTED`
- **AND** the row SHALL remain grayed out and non-actionable

#### Scenario: Archived 行はアーカイブ済み表示を優先する

- **GIVEN** the TUI is in parallel mode
- **AND** a change row is in `Archived` status
- **WHEN** the Changes list is rendered
- **THEN** the row SHALL display the archived checkbox styling (e.g., gray `[x]`)
- **AND** the row SHALL NOT display `UNCOMMITTED`

#### Scenario: active TUI contract uses the correct spelling

- **GIVEN** the Changes list renders an actual dirty-proposal badge
- **WHEN** the badge text is displayed
- **THEN** the text SHALL be `UNCOMMITTED`
- **AND** the text SHALL NOT be `UNCOMMITED`

### Requirement: Log Panel Toggle Hint

Changes ビューの Changes パネルはログパネルの切り替え操作として `l: logs` を表示しなければならない（SHALL）。

Logs パネルが表示されている場合、TUI はログ閲覧操作として `PageUp` / `PageDown`、`Home` / `End`、および `l` によるログパネル切り替えをユーザーが画面上で発見できるように表示しなければならない（SHALL）。表示は端末幅を圧迫しないように短縮表記を使ってよい（MAY）が、各操作の意味が分かる必要がある。

<!-- Expected canonical result after archive: `tui-key-hints` will require both the existing `l: logs` Changes-panel hint and visible Logs-panel navigation guidance for PageUp/PageDown, Home/End, and l. -->

#### Scenario: Select mode shows log toggle hint

- **GIVEN** TUI is in select mode
- **WHEN** Changes panel is rendered
- **THEN** key hints include "l: logs"

#### Scenario: Running mode shows log toggle hint

- **GIVEN** TUI is in running mode
- **WHEN** Changes panel is rendered
- **THEN** key hints include "l: logs"

#### Scenario: Logs panel shows scroll guidance

- **GIVEN** the TUI is rendering the Changes view
- **AND** the Logs panel is visible
- **WHEN** the Logs panel title or adjacent help area is rendered
- **THEN** the visible UI shows that `PageUp` / `PageDown` scroll older/newer log entries
- **AND** the visible UI shows that `Home` / `End` jump to the oldest/newest log position
- **AND** the visible UI shows that `l` toggles or hides the Logs panel

#### Scenario: Hidden Logs panel does not consume extra log-scroll help space

- **GIVEN** the TUI is rendering the Changes view
- **AND** the Logs panel is hidden
- **WHEN** the Changes view is rendered
- **THEN** the Changes panel still shows the existing `l: logs` toggle hint
- **AND** the UI is not required to reserve separate Logs-panel scroll guidance space

### Requirement: Active Change Stop Hint
Changes パネルは、カーソルが active change にある場合、`Space: stop` を表示しなければならない（SHALL）。

#### Scenario: Running mode shows Space: stop for active change
- **GIVEN** the TUI is in running mode
- **AND** the cursor is on a change with `queue_status.is_active() == true`
- **WHEN** the Changes panel is rendered
- **THEN** key hints include "Space: stop"

### Requirement: Bulk Toggle Key Hint

Changes パネルは、全マーク/全アンマークのトグルが有効な場合に `x: toggle all` を表示しなければならない（SHALL）。

Running/Stopping/Error の間は当該ヒントを表示してはならない（SHALL NOT）。

#### Scenario: Select モードでヒントを表示する
- **GIVEN** the TUI is in select mode
- **AND** at least one eligible change exists
- **WHEN** the Changes panel is rendered
- **THEN** key hints SHALL include "x: toggle all"

#### Scenario: Running モードでヒントを表示しない
- **GIVEN** the TUI is in running mode
- **WHEN** the Changes panel is rendered
- **THEN** key hints SHALL NOT include "x: toggle all"

### Requirement: Error Details Key Hint

The Changes panel SHALL display an `Enter: details` key hint when the cursor is on a change whose display status is `error`. The hint SHALL be available in both Select and Running Changes views and SHALL NOT be shown for a non-error row.

<!-- Expected canonical result after archive: `tui-key-hints` will advertise the error-details popup only when the focused Changes row can open it. -->

#### Scenario: Error row advertises details action

- **GIVEN** the TUI is rendering the Changes view
- **AND** the cursor is on a change whose display status is `error`
- **WHEN** the Changes panel key hints are rendered
- **THEN** the hints SHALL include `Enter: details`

#### Scenario: Non-error row does not advertise details action

- **GIVEN** the TUI is rendering the Changes view
- **AND** the cursor is on a change whose display status is not `error`
- **WHEN** the Changes panel key hints are rendered
- **THEN** the hints SHALL NOT include `Enter: details`
