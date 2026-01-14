# tui-key-hints Delta

## MODIFIED Requirements

### Requirement: Approval State Transition in Stopped Mode

停止モードで `MergeWait` の change が選択中の場合、`M` は選択中 change のみを解決する単発操作でなければならない（SHALL）。

さらに、`M` による merge resolve の実行中、TUI は対象 change のステータス表示を `resolving` として示し、TUI の描画ループをブロックしてはならない（SHALL NOT）。

#### Scenario: resolve 実行中は `resolving` を表示する
- **GIVEN** the TUI is in stopped mode
- **AND** the cursor is on a change in `MergeWait`
- **WHEN** the user presses `M`
- **THEN** the TUI SHALL set the selected change status to `Resolving` immediately
- **AND** the Changes list status text SHALL show `resolving` while the resolve operation is running
- **AND** the TUI SHALL continue rendering and accepting key input during the resolve operation
