## MODIFIED Requirements

### Requirement: Context-Aware Key Hints in Select Mode

`MergeWait` の change が選択中の場合、TUI は解決操作として `M` を提示しなければならない（SHALL）。Configured start key hints SHALL describe app-level orchestration start/resume/retry only.

Visible pre-archive row では execution mark の現在値に応じて `Space: mark` または `Space: unmark` を表示しなければならない（SHALL）。Active row では独立した `K: kill` hint と mark hint を同時に表示しなければならない（SHALL）。Archived、merged、または pushed row では Space mark hint を表示してはならない（MUST NOT）。

#### Scenario: pre-archive row は mark hint を表示する

- **GIVEN** the TUI is in select mode
- **AND** the cursor is on a visible pre-archive row
- **WHEN** the Changes list is rendered
- **THEN** an unmarked row shows `Space: mark`
- **AND** a marked row shows `Space: unmark`

#### Scenario: active row は kill と mark を独立表示する

- **GIVEN** the cursor is on an active pre-archive change
- **WHEN** the Changes list is rendered
- **THEN** the Changes panel shows `K: kill`
- **AND** it also shows `Space: mark` or `Space: unmark`

#### Scenario: post-archive row は mark hint を表示しない

- **GIVEN** the cursor is on an archived, merged, or pushed row
- **WHEN** the Changes list is rendered
- **THEN** the Changes panel does not show `Space: mark` or `Space: unmark`

<!-- Expected canonical result after archive: `tui-key-hints` will describe Space as pure mark intent and retain K as the independent active-work termination control. -->

### Requirement: Context-Aware Key Hints in Running Mode

The TUI SHALL display execution-mark hints in running mode consistent with select mode. Every visible pre-archive row SHALL show `Space: mark` or `Space: unmark` from current mark state regardless of active/retry/wait status or current eligibility. Active rows SHALL additionally show `K: kill`. Post-archive rows SHALL show no Space mark hint.

Changes panel title SHALL show only change-related keys. App-level control keys SHALL remain in the Status panel title.

#### Scenario: Running mode shows mark and independent kill keys

- **GIVEN** the TUI is in running mode
- **AND** the cursor is on an active pre-archive change
- **WHEN** the Changes panel key hints are rendered
- **THEN** the hints include `K: kill`
- **AND** the hints include `Space: mark` or `Space: unmark`
- **AND** they do not describe Space as queue, unqueue, retry, or stop

#### Scenario: Running mode with post-archive row omits mark hint

- **GIVEN** the TUI is in running mode
- **AND** the cursor is on an archived, merged, or pushed row
- **WHEN** the Changes panel key hints are rendered
- **THEN** no Space mark hint is shown

<!-- Expected canonical result after archive: `tui-key-hints` will make Running Space hints mark-only and remove queue/retry aliases. -->

### Requirement: Active Change Stop Hint

Changes パネルは、cursor が active change にある場合、per-change termination control として `K: kill` を表示しなければならない（SHALL）。Space は active change でも execution mark の変更に使用し、stop/dequeue control として表示してはならない（MUST NOT）。

#### Scenario: Running mode shows independent kill and mark hints for active change

- **GIVEN** the TUI is in running mode
- **AND** the cursor is on an active pre-archive change
- **WHEN** the Changes panel is rendered
- **THEN** key hints include `K: kill`
- **AND** key hints include `Space: mark` or `Space: unmark`
- **AND** key hints do not include `Space: stop`

<!-- Expected canonical result after archive: `tui-key-hints` will bind active change termination to K and reserve Space for mark intent. -->

### Requirement: Bulk Toggle Key Hint

Changes パネルは、visible pre-archive change が1件以上存在し、overlay が input を所有していない場合に `x: toggle all` を表示しなければならない（SHALL）。この hint は Select、Running、Stopping、Stopped、および Error の全 execution mode で表示可能でなければならない（SHALL）。Post-archive row だけが表示されている場合は hint を表示してはならない（MUST NOT）。

#### Scenario: 全 execution mode で hint を表示する

- **GIVEN** the TUI is in Select, Running, Stopping, Stopped, or Error mode
- **AND** at least one visible pre-archive change exists
- **AND** no overlay owns input
- **WHEN** the Changes panel is rendered
- **THEN** key hints include `x: toggle all`

#### Scenario: post-archive row だけなら hint を表示しない

- **GIVEN** every visible row is archived, merged, or pushed
- **WHEN** the Changes panel is rendered
- **THEN** key hints do not include `x: toggle all`

<!-- Expected canonical result after archive: `tui-key-hints` will base the bulk hint on visible pre-archive targets rather than execution mode or run eligibility. -->
