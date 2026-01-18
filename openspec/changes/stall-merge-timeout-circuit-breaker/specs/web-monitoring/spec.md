## ADDED Requirements
### Requirement: merge 停滞停止の通知

Web monitoring は merge 停滞による停止時に、停止理由を含む状態更新を配信しなければならない（SHALL）。

#### Scenario: merge 停滞による停止が発生した場合
- **GIVEN** web monitoring が有効である
- **AND** merge 停滞が検出される
- **WHEN** 停止イベントが発行される
- **THEN** 状態更新に merge 停滞の停止理由が含まれる
