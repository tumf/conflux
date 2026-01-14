## MODIFIED Requirements

### Requirement: Context-Aware Key Hints in Select Mode

`MergeWait` の change が選択中の場合、TUI は解決操作として `M` を提示しなければならない（SHALL）。

`MergeWait` 以外の change が選択中の場合、TUI は `M` 操作ヒントを表示してはならない（SHALL NOT）。

#### Scenario: `MergeWait` の行では `M` を表示する
- **GIVEN** the TUI is in select mode
- **AND** the cursor is on a change in `MergeWait` status
- **WHEN** the Changes list is rendered
- **THEN** the Changes panel key hints SHALL show "M: resolve"

### Requirement: App Control Keys in Status Panel Title

停止/待機中に `MergeWait` が存在する場合でも、TUI は自動で処理を再開する操作ヒントを追加してはならない（SHALL NOT）。

#### Scenario: `MergeWait` が存在しても自動再開のヒントは増やさない
- **GIVEN** the TUI is in stopped mode
- **AND** at least one change is in `MergeWait`
- **WHEN** the Status panel title is rendered
- **THEN** the title SHALL NOT imply automatic resume of merge

### Requirement: Approval State Transition in Stopped Mode

停止モードで `MergeWait` の change が選択中の場合、`M` は選択中 change のみを解決する単発操作でなければならない（SHALL）。

#### Scenario: `M` は選択中 change のみを対象とする
- **GIVEN** the TUI is in stopped mode
- **AND** the cursor is on a change in `MergeWait`
- **WHEN** the user presses `M`
- **THEN** the TUI SHALL trigger merge resolution for the selected change only
