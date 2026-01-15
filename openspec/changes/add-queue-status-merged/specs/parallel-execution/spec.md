# Parallel Execution Spec Delta

## MODIFIED Requirements

### Requirement: Parallel Execution Event Reporting

parallel 実行モジュールは、統一された `ExecutionEvent` 型を使用してイベントを発行しなければならない（SHALL）。

**変更内容**: `MergeCompleted` イベント受信時の TUI 側の状態遷移を `Archived` から **`Merged`** に変更する。これにより、並列モードでマージが完了した変更を明確に区別できる。

#### Scenario: マージ完了イベント受信時に Merged 状態に遷移

- **GIVEN** parallel executor が個別の change をマージする
- **WHEN** マージが成功する
- **THEN** `ExecutionEvent::MergeCompleted { change_id, revision }` が発行される
- **AND** TUIは `change_id` に該当する変更のステータスを **`Merged`** に設定する
- **AND** `Merged` 状態は "merged" として表示され、色は `Color::LightBlue` である

#### Scenario: 複数変更の逐次マージ時に各変更が Merged 状態に遷移

- **GIVEN** parallel executor が複数の change を逐次マージする
- **WHEN** 各変更のマージが成功する
- **THEN** 各変更ごとに `ExecutionEvent::MergeCompleted { change_id, revision }` が発行される
- **AND** 各変更のステータスが `Merged` に設定される
- **AND** TUI上で各変更が個別に "merged" として表示される
