## MODIFIED Requirements
### Requirement: 未コミット change は操作不可として表示する

未コミットの change は Changes パネルで操作不可の状態として表示しなければならない（SHALL）。

ここでいう未コミット change には、`HEAD` に存在しない change だけでなく、`openspec/changes/<change_id>/` 配下に未コミットまたは未追跡ファイルが存在する change も含まれる。

並列モードでは、未コミット change がユーザーに分かる形で表示されること。Archived 状態の行はアーカイブ済み表示を優先し、`UNCOMMITED` バッジを表示してはならない（SHALL NOT）。

#### Scenario: Archived 行には `UNCOMMITED` を表示しない

- **GIVEN** the TUI is in parallel mode
- **AND** a change row is in `Archived` status
- **WHEN** the Changes list is rendered
- **THEN** the row SHALL NOT display the `UNCOMMITED` badge
- **AND** the row SHALL display the archived checkbox styling (e.g., gray `[x]`)

#### Scenario: change 配下に未コミットがある行には `UNCOMMITED` を表示する

- **GIVEN** the TUI is in parallel mode
- **AND** a change row is eligible to be queued (e.g., `NotQueued` or `Queued`)
- **AND** the change has uncommitted or untracked files under `openspec/changes/<change_id>/`
- **WHEN** the Changes list is rendered
- **THEN** the row SHALL be grayed out
- **AND** the row SHALL NOT display a checkbox
- **AND** the row SHALL display the `UNCOMMITED` badge
