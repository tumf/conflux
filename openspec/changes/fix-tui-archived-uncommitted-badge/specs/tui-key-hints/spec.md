## MODIFIED Requirements

### Requirement: 並列モードの `UNCOMMITED` バッジ表示はキュー可能な行に限定する

TUI は並列モードにおいて、未コミット change をユーザーに分かる形で表示しなければならない（SHALL）。

ただし、`Archived` 状態の行は操作対象ではなく、アーカイブ済みであることを示す表示を優先するため、`UNCOMMITED` バッジを表示してはならない（SHALL NOT）。

#### Scenario: Archived 行には `UNCOMMITED` を表示しない

- **GIVEN** the TUI is in parallel mode
- **AND** a change row is in `Archived` status
- **WHEN** the Changes list is rendered
- **THEN** the row SHALL NOT display the `UNCOMMITED` badge
- **AND** the row SHALL display the archived checkbox styling (e.g., gray `[x]`)

#### Scenario: 未コミットかつキュー可能な行には `UNCOMMITED` を表示する

- **GIVEN** the TUI is in parallel mode
- **AND** a change row is eligible to be queued (e.g., `NotQueued` or `Queued`)
- **AND** the change is not parallel-eligible (uncommitted)
- **WHEN** the Changes list is rendered
- **THEN** the row SHALL display the `UNCOMMITED` badge
