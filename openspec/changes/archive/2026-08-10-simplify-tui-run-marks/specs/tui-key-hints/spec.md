## MODIFIED Requirements

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
