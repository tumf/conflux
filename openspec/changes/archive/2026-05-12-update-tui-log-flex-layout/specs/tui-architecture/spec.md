## MODIFIED Requirements

### Requirement: TUI Module Structure

TUI モジュールは `src/tui/` 配下のディレクトリ構成で整理され、TUI state 層は共有オーケストレーション状態から change の進捗と実行メタデータを取得しなければならない（SHALL）。UI 固有の状態（カーソル、ビュー、選択状態など）は TUI 側で保持する。
共有状態から取り込む iteration は、既に表示されている値より小さい場合に上書きしてはならない。表示された iteration が後退しないよう、より大きい値を維持しなければならない。
さらに、出力イベントにより iteration を更新する際は、現在の `queue_status` に一致するステージのイベントのみを反映し、同一ステージ内で iteration が単調増加となるように更新しなければならない。ステージ開始時は iteration 表示をリセットし、前ステージの値を持ち越してはならない。この更新規則は MUST とする。

Running mode の logs panel が有効な場合、TUI は changes list の現在の表示需要を超える余剰縦領域を logs panel に割り当てなければならない（MUST）。ただし、changes が多く list 領域を必要とする場合、logs panel は既存の 20 行相当の高さを維持しなければならない（MUST）。logs panel が無効な Running mode、および Select mode / Worktree view のレイアウトはこの動的再配分の対象外とする。

<!-- Expected canonical result after archive: `tui-architecture` will document that Running mode dynamically gives unused changes-list height to logs while preserving the existing logs height for large change sets. -->

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
