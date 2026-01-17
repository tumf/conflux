## MODIFIED Requirements
### Requirement: Event-Driven State Updates

TUI は実行イベントを受信して内部状態を更新しなければならない（SHALL）。

**変更内容**:
- resolve待ちの変更は `NotQueued` ではなく、待機状態として視覚的に識別できる状態で表示する
- resolve待ち状態はユーザーの明示操作がない限り、auto-refresh やリスト更新で消失しない

#### Scenario: resolve待ち状態の表示を維持する
- **GIVEN** 変更が merge 待機状態として記録されている
- **WHEN** TUI が変更リストを再描画する
- **THEN** 変更のステータスは resolve待ちとして表示される
- **AND** `NotQueued` として表示されない

#### Scenario: resolve待ち状態は自動更新で保持される
- **GIVEN** 変更が resolve待ち状態である
- **WHEN** TUI が変更一覧を更新する
- **THEN** 変更の状態は resolve待ちのまま保持される
- **AND** ユーザー操作がない限りキューから外れた表示にならない
