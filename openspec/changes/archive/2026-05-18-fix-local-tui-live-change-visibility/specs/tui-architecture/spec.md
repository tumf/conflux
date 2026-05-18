## MODIFIED Requirements

### Requirement: TUI Module Structure

TUI モジュールは `src/tui/` 配下のディレクトリ構成で整理され、TUI state 層は共有オーケストレーション状態から change の進捗と実行メタデータを取得しなければならない（SHALL）。UI 固有の状態（カーソル、ビュー、選択状態など）は TUI 側で保持する。
共有状態から取り込む iteration は、既に表示されている値より小さい場合に上書きしてはならない。表示された iteration が後退しないよう、より大きい値を維持しなければならない。
さらに、出力イベントにより iteration を更新する際は、現在の `queue_status` に一致するステージのイベントのみを反映し、同一ステージ内で iteration が単調増加となるように更新しなければならない。ステージ開始時は iteration 表示をリセットし、前ステージの値を持ち越してはならない。この更新規則は MUST とする。

Running mode の logs panel が有効な場合、TUI は changes list の現在の表示需要を超える余剰縦領域を logs panel に割り当てなければならない（MUST）。ただし、changes が多く list 領域を必要とする場合、logs panel は既存の 20 行相当の高さを維持しなければならない（MUST）。logs panel が無効な Running mode、および Select mode / Worktree view のレイアウトはこの動的再配分の対象外とする。

Local TUI mode の auto-refresh は、起動時に確定した repository root を基準に active changes と rejected marker rows を読み取らなければならない（MUST）。Auto-refresh は process current working directory の後続変化に依存して change discovery を行ってはならない（MUST NOT）。

Running mode は、auto-refresh により新規 active change が検出され `new_change_count > 0` になった場合、追加された row が現在の changes list viewport 外にあってもユーザーが新規 change の存在を認識できる表示シグナルを出さなければならない（MUST）。この表示シグナルは observability-only であり、queue、scheduler dispatch、resume routing、acceptance、archive、next-action decision の入力として使ってはならない（MUST NOT）。

<!-- Expected canonical result after archive: `tui-architecture` will require local auto-refresh to use captured repo_root for change discovery and require Running mode to surface a new-change signal even when the appended row is off-screen. -->

#### Scenario: Few changes expand logs panel

- **GIVEN** the TUI is rendering Running mode with the logs panel enabled
- **AND** the terminal has more vertical space than the current changes visual rows need
- **WHEN** the Running mode layout is computed
- **THEN** the logs panel receives the surplus vertical space beyond the changes list display need
- **AND** the logs panel is taller than the existing fixed 20-row allocation

#### Scenario: Many changes preserve current logs height

- **GIVEN** the TUI is rendering Running mode with the logs panel enabled
- **AND** the current changes visual rows need the available flexible changes-list area
- **WHEN** the Running mode layout is computed
- **THEN** the logs panel keeps the existing 20-row allocation
- **AND** the changes list uses the remaining vertical space

#### Scenario: Logs disabled layout remains unchanged

- **GIVEN** the TUI is rendering Running mode with the logs panel disabled
- **WHEN** the Running mode layout is computed
- **THEN** only the header, changes list, and status areas are rendered
- **AND** the changes list continues to receive the flexible remaining area

#### Scenario: Local refresh uses captured repository root

- **GIVEN** local TUI mode started from repository root `/repo`
- **AND** the process current working directory later differs from `/repo`
- **AND** `/repo/openspec/changes/new-visible/proposal.md` exists
- **WHEN** the auto-refresh task scans changes
- **THEN** `new-visible` is discovered from `/repo/openspec/changes`
- **AND** discovery does not depend on the later process current working directory

#### Scenario: Running mode surfaces off-screen new change

- **GIVEN** the TUI is in Running mode with the logs panel enabled
- **AND** enough changes exist that a newly appended row may be outside the visible changes-list viewport
- **WHEN** auto-refresh discovers a new active change
- **THEN** Running mode displays a visible new-change signal such as `New: 1`
- **AND** the signal remains visible even if the new row itself is not currently visible
- **AND** the cursor is not moved solely due to the discovery

#### Scenario: New-change signal does not control workflow

- **GIVEN** Running mode displays a new-change signal for change `new-visible`
- **WHEN** scheduler routing, queue membership, resume routing, acceptance, or archive decisions are evaluated
- **THEN** the display signal and any associated TUI log are ignored as workflow-control inputs
- **AND** `new-visible` is not selected or queued unless the user explicitly marks it
