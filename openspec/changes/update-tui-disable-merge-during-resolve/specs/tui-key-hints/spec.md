## MODIFIED Requirements
### Requirement: Context-Aware Key Hints in Select Mode

`MergeWait` の change が選択中の場合、TUI は解決操作として `M` を提示しなければならない（SHALL）。ただし、resolve実行中は `M` を提示してはならない（SHALL NOT）。

`MergeWait` 以外の change が選択中の場合、TUI は `M` 操作ヒントを表示してはならない（SHALL NOT）。

#### Scenario: resolve中は `M` を表示しない
- **GIVEN** the TUI is in select mode
- **AND** the cursor is on a change in `MergeWait` status
- **AND** a resolve operation is in progress
- **WHEN** the Changes list is rendered
- **THEN** the Changes panel key hints SHALL NOT show "M: resolve"

## MODIFIED Requirements
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
