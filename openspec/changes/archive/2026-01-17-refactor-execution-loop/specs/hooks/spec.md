## MODIFIED Requirements

### Requirement: TUI and CLI hook parity

オーケストレーターは、TUI モードと CLI（run）モードで同一のフックを同一のコンテキストで実行しなければならない（SHALL）。

#### Scenario: CLI で hook 実行イベントを通知する
- **GIVEN** hooks が設定されており CLI（run）モードで change が処理中である
- **WHEN** apply/archive 中に hook が開始・完了する
- **THEN** hook 実行は parallel と同一のイベント通知で報告される
- **AND** hook 実行順序はライフサイクル定義に従う
