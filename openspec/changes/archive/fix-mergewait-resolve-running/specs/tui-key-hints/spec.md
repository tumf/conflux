## MODIFIED Requirements
### Requirement: Approval State Transition in Running Mode

Runningモードで`MergeWait`のchangeが選択中の場合、`M`によるmerge resolveを開始できなければならない（SHALL）。

#### Scenario: RunningモードでMergeWaitのchangeをresolveできる
- **GIVEN** the TUI is in running mode
- **AND** the cursor is on a change in `MergeWait` status
- **AND** a resolve operation is not in progress
- **WHEN** the user presses `M`
- **THEN** the resolve command SHALL be triggered for the selected change
- **AND** the change status SHALL transition to `resolving` while the resolve runs
