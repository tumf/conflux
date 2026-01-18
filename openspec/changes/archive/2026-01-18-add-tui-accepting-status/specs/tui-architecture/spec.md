## MODIFIED Requirements
### Requirement: Event-Driven State Updates
TUI は実行イベントを受信して内部状態を更新しなければならない（SHALL）。
acceptance 実行が開始された場合、TUI は該当 change のステータスを `accepting` として表示しなければならない（SHALL）。
acceptance 実行が完了した場合、TUI は既存のステータス遷移に戻さなければならない（SHALL）。

#### Scenario: acceptance 実行中のステータス更新
- **GIVEN** TUI が change の状態を表示している
- **AND** acceptance 実行が開始される
- **WHEN** acceptance 開始イベントを受信する
- **THEN** 該当 change のステータスは `accepting` として表示される
- **AND** acceptance 完了イベントを受信した後は既存の遷移状態に戻る
