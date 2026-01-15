# parallel-execution Specification Delta

## MODIFIED Requirements

### Requirement: Parallel Execution Event Reporting

parallel 実行モジュールは、統一された `ExecutionEvent` 型を使用してイベントを発行しなければならない（SHALL）。

**変更内容**: `MergeCompleted` イベントに `change_id` フィールドを追加し、TUIがどの変更のマージが完了したかを特定できるようにする。

#### Scenario: マージ完了イベントに change_id を含める

- **GIVEN** parallel executor が個別の change をマージする
- **WHEN** マージが成功する
- **THEN** `ExecutionEvent::MergeCompleted { change_id, revision }` が発行される
- **AND** `change_id` にはマージされた変更のIDが含まれる
- **AND** TUIはこのイベントを受け取って当該変更のステータスを更新できる

#### Scenario: 複数変更の逐次マージ時に各 MergeCompleted を発行

- **GIVEN** parallel executor が複数の change を逐次マージする
- **WHEN** 各変更のマージが成功する
- **THEN** 各変更ごとに `ExecutionEvent::MergeCompleted { change_id, revision }` が発行される
- **AND** 全ての change_id がイベントに含まれる
