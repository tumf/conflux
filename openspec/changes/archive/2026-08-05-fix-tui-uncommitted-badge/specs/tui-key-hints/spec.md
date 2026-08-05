## MODIFIED Requirements

### Requirement: 未コミット change の操作ヒントを非表示にする

並列モードで `openspec/changes/<change_id>/` 配下に未コミットまたは未追跡ファイルがある change が選択中の場合、Changes パネルのキーヒントは選択に関する操作を表示してはならない（SHALL）。

`HEAD` に proposal directory が存在しないことだけを未コミット状態として表示してはならない（SHALL NOT）。ただし、proposal absence による既存の parallel queue admission rule は維持しなければならない（SHALL）。

<!-- Expected canonical result after archive: `tui-key-hints` will distinguish dirty proposal content from proposal absence when deciding whether to describe a row as uncommitted, without weakening parallel admission. -->

#### Scenario: 未コミット proposal は選択ヒントを表示しない

- **GIVEN** TUI が並列モードで表示されている
- **AND** カーソル位置の change の proposal directory に未コミットまたは未追跡ファイルがある
- **WHEN** Changes パネルを描画する
- **THEN** `Space: queue` のキーヒントは表示されない

#### Scenario: proposal absence は未コミット理由として扱わない

- **GIVEN** TUI が並列モードで表示されている
- **AND** カーソル位置の change の proposal directory は `HEAD` に存在しない
- **AND** その change の proposal directory に未コミットまたは未追跡ファイルはない
- **WHEN** Changes パネルを描画する
- **THEN** TUI はその行を未コミットであるとは表示しない
- **AND** the row remains grayed out, non-markable, and without queue affordances
- **AND** proposal absence による parallel queue admission refusal は維持される
- **AND** any refusal message identifies proposal absence from `HEAD` instead of instructing the operator to commit nonexistent dirty content

### Requirement: 未コミット change は操作不可として表示する

`openspec/changes/<change_id>/` 配下に未コミットまたは未追跡ファイルがある active change は、Changes パネルで操作不可の状態として表示しなければならない（SHALL）。

並列実行への不適格理由は区別されなければならない（SHALL）。proposal directory が `HEAD` に存在しないことだけを、未コミットまたは未追跡ファイルの存在として表示してはならない（SHALL NOT）。

実際に未コミットまたは未追跡の proposal ファイルが存在する場合のみ、TUI は正しい綴りの `UNCOMMITTED` バッジを表示しなければならない（SHALL）。Archived 状態、failed merge 状態、または retained managed worktree の存在だけを理由に `UNCOMMITTED` を表示してはならない（SHALL NOT）。managed worktree が存在する場合の `WT` 表示は独立して維持されなければならない（SHALL）。

<!-- Expected canonical result after archive: `tui-key-hints` will reserve `UNCOMMITTED` for observed dirty proposal files, retain independent `WT` display, and stop equating all parallel-ineligible reasons with dirty Git state. -->

#### Scenario: dirty active proposal は `UNCOMMITTED` を表示する

- **GIVEN** the TUI is in parallel mode
- **AND** a change row is in a queueable active status such as `NotQueued` or `Queued`
- **AND** the change has uncommitted or untracked files under `openspec/changes/<change_id>/`
- **WHEN** the Changes list is rendered
- **THEN** the row SHALL be grayed out and non-actionable
- **AND** the row SHALL display `UNCOMMITTED`

#### Scenario: clean proposal absence は `UNCOMMITTED` を表示しない

- **GIVEN** the TUI is in parallel mode
- **AND** a change is parallel-ineligible because its proposal directory is absent from `HEAD`
- **AND** no uncommitted or untracked proposal files are observed for that change
- **WHEN** the Changes list is rendered
- **THEN** the row SHALL NOT display `UNCOMMITTED`
- **AND** the row SHALL remain grayed out, non-markable, and without queue affordances
- **AND** the change SHALL remain ineligible for parallel queue admission

#### Scenario: clean proposal absence has a truthful refusal reason

- **GIVEN** a parallel-ineligible change has no proposal directory in `HEAD`
- **AND** no uncommitted or untracked proposal files are observed for that change
- **WHEN** a single-row or bulk-toggle operation reports why the change was excluded
- **THEN** the reason SHALL identify that the change is not present in `HEAD`
- **AND** the reason SHALL NOT describe the change as uncommitted or instruct the operator to commit it

#### Scenario: retained worktree marker is independent of dirty state

- **GIVEN** an archived or failed-merge change retains a clean managed worktree
- **AND** its active proposal directory is absent from `HEAD`
- **AND** the change row is displayed in a queueable status such as `NotQueued` or `Queued`
- **WHEN** the Changes list is rendered
- **THEN** the row MAY display `WT`
- **AND** the row SHALL NOT display `UNCOMMITTED`
- **AND** the row SHALL remain grayed out and non-actionable

#### Scenario: Archived 行はアーカイブ済み表示を優先する

- **GIVEN** the TUI is in parallel mode
- **AND** a change row is in `Archived` status
- **WHEN** the Changes list is rendered
- **THEN** the row SHALL display the archived checkbox styling (e.g., gray `[x]`)
- **AND** the row SHALL NOT display `UNCOMMITTED`

#### Scenario: active TUI contract uses the correct spelling

- **GIVEN** the Changes list renders an actual dirty-proposal badge
- **WHEN** the badge text is displayed
- **THEN** the text SHALL be `UNCOMMITTED`
- **AND** the text SHALL NOT be `UNCOMMITED`
