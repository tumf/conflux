## MODIFIED Requirements

### Requirement: Log Panel Toggle Hint

Changes ビューの Changes パネルはログパネルの切り替え操作として `l: logs` を表示しなければならない（SHALL）。

Logs パネルが表示されている場合、TUI はログ閲覧操作として `PageUp` / `PageDown`、`Home` / `End`、および `l` によるログパネル切り替えをユーザーが画面上で発見できるように表示しなければならない（SHALL）。表示は端末幅を圧迫しないように短縮表記を使ってよい（MAY）が、各操作の意味が分かる必要がある。

<!-- Expected canonical result after archive: `tui-key-hints` will require both the existing `l: logs` Changes-panel hint and visible Logs-panel navigation guidance for PageUp/PageDown, Home/End, and l. -->

#### Scenario: Select mode shows log toggle hint

- **GIVEN** TUI is in select mode
- **WHEN** Changes panel is rendered
- **THEN** key hints include "l: logs"

#### Scenario: Running mode shows log toggle hint

- **GIVEN** TUI is in running mode
- **WHEN** Changes panel is rendered
- **THEN** key hints include "l: logs"

#### Scenario: Logs panel shows scroll guidance

- **GIVEN** the TUI is rendering the Changes view
- **AND** the Logs panel is visible
- **WHEN** the Logs panel title or adjacent help area is rendered
- **THEN** the visible UI shows that `PageUp` / `PageDown` scroll older/newer log entries
- **AND** the visible UI shows that `Home` / `End` jump to the oldest/newest log position
- **AND** the visible UI shows that `l` toggles or hides the Logs panel

#### Scenario: Hidden Logs panel does not consume extra log-scroll help space

- **GIVEN** the TUI is rendering the Changes view
- **AND** the Logs panel is hidden
- **WHEN** the Changes view is rendered
- **THEN** the Changes panel still shows the existing `l: logs` toggle hint
- **AND** the UI is not required to reserve separate Logs-panel scroll guidance space
