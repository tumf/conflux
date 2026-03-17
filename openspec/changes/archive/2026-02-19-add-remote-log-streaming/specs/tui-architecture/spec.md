## MODIFIED Requirements
### Requirement: リモート更新の購読
TUI は `--server` が指定された場合、リモートサーバの状態更新を購読し、既存の iteration 非後退ルールに従って反映しなければならない（MUST）。

リモート更新にはログイベントが含まれる場合があり、TUI はログパネルと change 行のログプレビューに反映しなければならない（MUST）。

#### Scenario: 古い iteration で上書きしない
- **GIVEN** TUI が `iteration_number=3` を表示している
- **WHEN** リモート更新で `iteration_number=2` が届く
- **THEN** TUI は `iteration_number=3` を保持する

#### Scenario: リモートログがログパネルに表示される
- **GIVEN** TUI が `--server` でリモートに接続している
- **WHEN** WebSocket でログイベントが届く
- **THEN** TUI のログパネルにログが表示される
