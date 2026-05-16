## MODIFIED Requirements

### Requirement: Context-Aware Key Hints in Select Mode

`MergeWait` の change が選択中の場合、TUI は解決操作として `M` を提示しなければならない（SHALL）。

resolve 実行中は `M: queue resolve` を表示し、resolve 未実行中は `M: resolve` を表示しなければならない（SHALL）。

`MergeWait` 以外の change が選択中の場合、TUI は `M` 操作ヒントを表示してはならない（SHALL NOT）。

Configured start key hints SHALL describe app-level orchestration start/resume/retry only and MUST NOT imply cursor-local `MergeWait` resolve behavior. When no TUI config override exists, the configured start key label is `F5`.

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

- **GIVEN** the resolved TUI start keybindings are `F5` and `r`
- **AND** marked runnable work exists
- **WHEN** the Changes panel key hints are rendered
- **THEN** the start-control hint SHALL use the label `F5/r`
- **AND** the hint SHALL describe app-level run/resume/retry behavior

### Requirement: App Control Keys in Status Panel Title

停止/待機中に `MergeWait` が存在する場合でも、TUI は自動で処理を再開する操作ヒントを追加してはならない（SHALL NOT）。

Status panel app-control hints SHALL use the resolved TUI start key label instead of hardcoded `F5` text.

<!-- Expected canonical result after archive: `tui-key-hints` will require status panel titles to render resolved start key labels such as `F5` or `F5/r`. -->

#### Scenario: `MergeWait` が存在しても自動再開のヒントは増やさない

- **GIVEN** the TUI is in stopped mode
- **AND** at least one change is in `MergeWait`
- **WHEN** the Status panel title is rendered
- **THEN** the title SHALL NOT imply automatic resume of merge

#### Scenario: Stopped mode status title uses configured start label

- **GIVEN** the resolved TUI start keybindings are `F5` and `r`
- **AND** the TUI is in stopped mode
- **WHEN** the Status panel title is rendered
- **THEN** the title SHALL include `F5/r: resume`

#### Scenario: Stopping mode status title uses configured start label

- **GIVEN** the resolved TUI start keybindings are `F5` and `r`
- **AND** the TUI is in stopping mode
- **WHEN** the Status panel title is rendered
- **THEN** the title SHALL include `F5/r: continue`
