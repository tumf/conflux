## ADDED Requirements
### Requirement: merge 停滞による強制停止メッセージ

CLI は merge 停滞による強制停止時に、停止理由を明示したメッセージを表示しなければならない（SHALL）。

#### Scenario: run モードで merge 停滞を検知した場合
- **GIVEN** `cflx run` が実行中である
- **AND** merge 停滞が検出される
- **WHEN** オーケストレーターが停止する
- **THEN** CLI は merge 停滞による停止メッセージを表示する
