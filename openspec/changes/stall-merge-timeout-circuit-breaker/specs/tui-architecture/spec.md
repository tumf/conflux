## ADDED Requirements
### Requirement: merge 停滞による停止表示

TUI は merge 停滞による停止時に、停止理由を表示しなければならない（SHALL）。

#### Scenario: 走行中に merge 停滞が検出された場合
- **GIVEN** TUI が実行モードである
- **AND** merge 停滞が検出される
- **WHEN** オーケストレーターが停止する
- **THEN** TUI は merge 停滞による停止理由を表示する
