## MODIFIED Requirements

### Requirement: Context-Aware Key Hints in Select Mode

`MergeWait` の change が選択中の場合、TUI は resolve 実行中なら `M: queue resolve`、未実行なら `M: resolve` を表示しなければならない（SHALL）。`MergeWait` 以外の change では `M` hint を表示してはならない（SHALL NOT）。Configured start key hints SHALL describe app-level orchestration start/resume/retry only, MUST NOT imply cursor-local resolve behavior, and SHALL use the resolved key label such as default `F5/!`.

Visible non-terminal row のうち reducer が archive 完了を記録していない markable row では execution mark の現在値に応じて `Space: mark` または `Space: unmark` を表示しなければならない（SHALL）。Active markable row では独立した `K: kill` hint と mark hint を同時に表示しなければならない（SHALL）。Terminal display status、または reducer-recorded archive-complete row では Space mark hint を表示してはならない（MUST NOT）。

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

#### Scenario: markable non-terminal row shows mark hint

- **GIVEN** the cursor is on a visible non-terminal row without reducer-recorded archive completion
- **WHEN** key hints are rendered
- **THEN** an unmarked row shows `Space: mark`
- **AND** a marked row shows `Space: unmark`

#### Scenario: active markable row shows independent kill and mark hints

- **GIVEN** the cursor is on an active non-terminal change without reducer-recorded archive completion
- **WHEN** key hints are rendered
- **THEN** the Changes panel shows `K: kill`
- **AND** it also shows `Space: mark` or `Space: unmark`

#### Scenario: non-markable row omits mark hint

- **GIVEN** the cursor is on a row with terminal display status or reducer-recorded archive completion
- **WHEN** key hints are rendered
- **THEN** the Changes panel does not show `Space: mark` or `Space: unmark`

#### Scenario: Configured start key label is rendered

- **GIVEN** resolved start keybindings are `F5` and `!`
- **AND** marked runnable work exists
- **WHEN** start-control hints are rendered
- **THEN** the hint uses `F5/!`
- **AND** describes app-level run/resume/retry behavior

### Requirement: Context-Aware Key Hints in Running Mode

The TUI SHALL display execution-mark hints in Running mode consistent with Select mode. Every visible non-terminal row without reducer-recorded archive completion SHALL show `Space: mark` or `Space: unmark` from current mark state regardless of active/retry/wait status or current eligibility. Active rows SHALL additionally show `K: kill` independently of markability. Rows with terminal display status or reducer-recorded archive completion SHALL show no Space mark hint.

Changes panel title SHALL show only change-related keys. App-level controls such as Esc and Ctrl+C SHALL remain in the Status panel title.

#### Scenario: Running mode shows mark and independent kill keys

- **GIVEN** the TUI is in Running mode
- **AND** cursor row is an active non-terminal change without reducer-recorded archive completion
- **WHEN** Changes panel hints are rendered
- **THEN** hints include `K: kill`
- **AND** include `Space: mark` or `Space: unmark`
- **AND** do not describe Space as queue, unqueue, retry, or stop
- **AND** do not show `Esc: stop` or `q: quit` in the Changes panel title

#### Scenario: Running non-markable row omits mark hint

- **GIVEN** the TUI is in Running mode
- **AND** cursor row has terminal display status or reducer-recorded archive completion
- **WHEN** Changes panel hints are rendered
- **THEN** no Space mark hint is shown

#### Scenario: Running mode with empty list

- **GIVEN** the TUI is in Running mode
- **WHEN** the Changes list is empty
- **THEN** the Changes panel shows no selection keys
- **AND** does not show `Esc: stop` or `q: quit`

### Requirement: 未コミット change は操作不可として表示する

`openspec/changes/<change_id>/` 配下に未コミットまたは未追跡ファイルがある active change は、Changes パネルで操作不可の状態として表示しなければならない（SHALL）。

並列実行への不適格理由は区別されなければならない（SHALL）。proposal directory が `HEAD` に存在しないことだけを、未コミットまたは未追跡ファイルの存在として表示してはならない（SHALL NOT）。

実際に未コミットまたは未追跡の proposal ファイルが存在する場合のみ、TUI は正しい綴りの `UNCOMMITTED` バッジを表示しなければならない（SHALL）。Archived 状態、failed merge 状態、または retained managed worktree の存在だけを理由に `UNCOMMITTED` を表示してはならない（SHALL NOT）。managed worktree が存在する場合の `WT` 表示は独立して維持されなければならない（SHALL）。Archived row の checkbox area は CLI の post-archive placeholder contract に従わなければならず（SHALL）、archived `[x]` styling を要求してはならない（MUST NOT）。

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

#### Scenario: Archived 行はblank checkbox placeholderを優先する

- **GIVEN** the TUI is in parallel mode
- **AND** a change row is in `Archived` status
- **WHEN** the Changes list is rendered
- **THEN** the row SHALL display the blank three-column checkbox placeholder required by the CLI specification
- **AND** the row SHALL NOT display archived `[x]` styling
- **AND** the row SHALL NOT display `UNCOMMITTED`

#### Scenario: active TUI contract uses the correct spelling

- **GIVEN** the Changes list renders an actual dirty-proposal badge
- **WHEN** the badge text is displayed
- **THEN** the text SHALL be `UNCOMMITTED`
- **AND** the text SHALL NOT be `UNCOMMITED`

### Requirement: Active Change Stop Hint

Changes パネルは、cursor が active change にある場合、per-change termination control として `K: kill` を表示しなければならない（SHALL）。Space は markable active change でのみ execution mark の変更に使用し、stop/dequeue control として表示してはならない（MUST NOT）。Archive-complete active change でも `K: kill` は markability と独立して表示しなければならない（SHALL）。

#### Scenario: Running mode shows independent kill and mark hints for markable active change

- **GIVEN** the TUI is in Running mode
- **AND** cursor row is an active non-terminal change without reducer-recorded archive completion
- **WHEN** the Changes panel is rendered
- **THEN** key hints include `K: kill`
- **AND** key hints include `Space: mark` or `Space: unmark`
- **AND** key hints do not include `Space: stop`

#### Scenario: archive-complete active change retains kill without mark hint

- **GIVEN** the TUI is in Running mode
- **AND** cursor row is active with reducer-recorded archive completion
- **WHEN** the Changes panel is rendered
- **THEN** key hints include `K: kill`
- **AND** key hints do not include `Space: mark`, `Space: unmark`, or `Space: stop`

### Requirement: Bulk Toggle Key Hint

Changes パネルは、reducer が archive 完了を記録していない visible non-terminal change が1件以上存在し、overlay が input を所有していない場合に `x: toggle all` を表示しなければならない（SHALL）。この hint は Select、Running、Stopping、Stopped、および Error の全 execution mode で表示可能でなければならない（SHALL）。Terminal display status または reducer-recorded archive-complete rows だけが表示されている場合は hint を表示してはならない（MUST NOT）。

#### Scenario: 全 execution mode で markable row があれば hint を表示する

- **GIVEN** the TUI is in Select, Running, Stopping, Stopped, or Error mode
- **AND** at least one visible non-terminal change without reducer-recorded archive completion exists
- **AND** no overlay owns input
- **WHEN** the Changes panel is rendered
- **THEN** key hints include `x: toggle all`

#### Scenario: non-markable rows だけなら hint を表示しない

- **GIVEN** every visible row has terminal display status or reducer-recorded archive completion
- **WHEN** the Changes panel is rendered
- **THEN** key hints do not include `x: toggle all`

<!-- Expected canonical result after archive: `tui-key-hints` will preserve resolve, start, kill, and dirty-worktree guidance while deriving mark hints from the shared markable-row class and replacing stale archived `[x]` styling with the CLI blank placeholder. -->
