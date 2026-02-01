## MODIFIED Requirements
### Requirement: Context-Aware Key Hints in Select Mode

`MergeWait` の change が選択中の場合、TUI は解決操作として `M` を提示しなければならない（SHALL）。ただし、resolve 操作が実行可能な状態に限る。

`MergeWait` 以外の change が選択中の場合、TUI は `M` 操作ヒントを表示してはならない（SHALL NOT）。

resolve 操作が実行不可能な場合（例: resolve 実行中、または該当モードで resolve が許可されない場合）、`MergeWait` の change が選択中でも `M` 操作ヒントを表示してはならない（SHALL NOT）。

#### Scenario: `MergeWait` の行では `M` を表示する
- **GIVEN** the TUI is in select mode
- **AND** the cursor is on a change in `MergeWait` status
- **AND** resolve operation is available
- **WHEN** the Changes list is rendered
- **THEN** the Changes panel key hints SHALL show "M: resolve"

#### Scenario: resolve が実行できない場合は `M` を表示しない
- **GIVEN** the TUI is in select mode
- **AND** the cursor is on a change in `MergeWait` status
- **AND** resolve operation is not available
- **WHEN** the Changes list is rendered
- **THEN** the Changes panel key hints SHALL NOT show "M: resolve"
