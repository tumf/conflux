# tui-key-hints Specification

## Purpose
Defines TUI key binding hints display based on application mode.
## Requirements

### Requirement: Context-Aware Key Hints in Select Mode

`MergeWait` の change が選択中の場合、TUI は resolve 実行中なら `M: queue resolve`、未実行なら `M: resolve` を表示しなければならない（SHALL）。`MergeWait` 以外の change では `M` hint を表示してはならない（SHALL NOT）。Configured start key hints SHALL describe app-level orchestration start/resume/retry only, MUST NOT imply cursor-local resolve behavior, and SHALL use the resolved key label such as default `F5/!`.

Visible non-terminal row では execution mark の現在値に応じて `Space: mark` または `Space: unmark` を表示しなければならない（SHALL）。Active row では独立した `K: kill` hint と mark hint を同時に表示しなければならない（SHALL）。Archived、merged、pushed、または rejected row では Space mark hint を表示してはならない（MUST NOT）。

#### Scenario: MergeWait shows the appropriate M hint

- **GIVEN** the TUI is in Select mode
- **AND** cursor row is in `MergeWait`
- **WHEN** key hints are rendered
- **THEN** the Changes panel shows `M: resolve` or `M: queue resolve` according to resolve activity
- **AND** configured start hints remain independent app-level orchestration controls

#### Scenario: non-MergeWait row does not show M

- **GIVEN** cursor row is not in `MergeWait`
- **WHEN** key hints are rendered
- **THEN** no M resolve hint is shown
- **AND** configured start availability for marked runnable work is unaffected

#### Scenario: non-terminal row shows mark hint

- **GIVEN** the cursor is on a visible non-terminal row
- **WHEN** key hints are rendered
- **THEN** an unmarked row shows `Space: mark`
- **AND** a marked row shows `Space: unmark`

#### Scenario: active row shows independent kill and mark hints

- **GIVEN** the cursor is on an active non-terminal change
- **WHEN** key hints are rendered
- **THEN** the Changes panel shows `K: kill`
- **AND** it also shows `Space: mark` or `Space: unmark`

#### Scenario: terminal row omits mark hint

- **GIVEN** the cursor is on an archived, merged, pushed, or rejected row
- **WHEN** key hints are rendered
- **THEN** the Changes panel does not show `Space: mark` or `Space: unmark`

#### Scenario: Configured start key label is rendered

- **GIVEN** resolved start keybindings are `F5` and `!`
- **AND** marked runnable work exists
- **WHEN** start-control hints are rendered
- **THEN** the hint uses `F5/!`
- **AND** describes app-level run/resume/retry behavior

### Requirement: Context-Aware Key Hints in Running Mode

The TUI SHALL display execution-mark hints in Running mode consistent with Select mode. Every visible non-terminal row SHALL show `Space: mark` or `Space: unmark` from current mark state regardless of active/retry/wait status or current eligibility. Active rows SHALL additionally show `K: kill`. Archived, merged, pushed, and rejected rows SHALL show no Space mark hint.

Changes panel title SHALL show only change-related keys. App-level controls such as Esc and Ctrl+C SHALL remain in the Status panel title.

#### Scenario: Running mode shows mark and independent kill keys

- **GIVEN** the TUI is in Running mode
- **AND** cursor row is an active non-terminal change
- **WHEN** Changes panel hints are rendered
- **THEN** hints include `K: kill`
- **AND** include `Space: mark` or `Space: unmark`
- **AND** do not describe Space as queue, unqueue, retry, or stop
- **AND** do not show `Esc: stop` or `q: quit` in the Changes panel title

#### Scenario: Running terminal row omits mark hint

- **GIVEN** the TUI is in Running mode
- **AND** cursor row is archived, merged, pushed, or rejected
- **WHEN** Changes panel hints are rendered
- **THEN** no Space mark hint is shown

#### Scenario: Running mode with empty list

- **GIVEN** the TUI is in Running mode
- **WHEN** the Changes list is empty
- **THEN** the Changes panel shows no selection keys
- **AND** does not show `Esc: stop` or `q: quit`

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

Changes パネルは、cursor が active change にある場合、per-change termination control として `K: kill` を表示しなければならない（SHALL）。Space は active change でも execution mark の変更に使用し、stop/dequeue control として表示してはならない（MUST NOT）。

#### Scenario: Running mode shows independent kill and mark hints for active change

- **GIVEN** the TUI is in Running mode
- **AND** cursor row is an active non-terminal change
- **WHEN** the Changes panel is rendered
- **THEN** key hints include `K: kill`
- **AND** key hints include `Space: mark` or `Space: unmark`
- **AND** key hints do not include `Space: stop`

### Requirement: Bulk Toggle Key Hint

Changes パネルは、visible non-terminal change が1件以上存在し、overlay が input を所有していない場合に `x: toggle all` を表示しなければならない（SHALL）。この hint は Select、Running、Stopping、Stopped、および Error の全 execution mode で表示可能でなければならない（SHALL）。Archived、merged、pushed、または rejected rows だけが表示されている場合は hint を表示してはならない（MUST NOT）。

#### Scenario: 全 execution mode で hint を表示する

- **GIVEN** the TUI is in Select, Running, Stopping, Stopped, or Error mode
- **AND** at least one visible non-terminal change exists
- **AND** no overlay owns input
- **WHEN** the Changes panel is rendered
- **THEN** key hints include `x: toggle all`

#### Scenario: terminal rows だけなら hint を表示しない

- **GIVEN** every visible row is archived, merged, pushed, or rejected
- **WHEN** the Changes panel is rendered
- **THEN** key hints do not include `x: toggle all`

<!-- Expected canonical result after archive: `tui-key-hints` will preserve M and configured-start guarantees while describing Space as pure mark intent, K as independent termination, and terminal rows as non-markable. -->

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
