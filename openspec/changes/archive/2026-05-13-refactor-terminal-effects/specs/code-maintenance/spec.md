## MODIFIED Requirements

### Requirement: リファクタリング安全性の担保

オーケストレーターはリファクタリング後も既存仕様の挙動を保ち、検証手順で後退がないことを示すために SHALL 検証を通過しなければならない。状態 reducer の副作用整理では、terminal/activity/wait state と resolve/reject wait queue の組み合わせを characterization test で固定しなければならない。

#### Scenario: reducer command の状態遷移が維持される

- **GIVEN** 既存の `OrchestratorState` と代表的な `ReducerCommand` がある
- **WHEN** command を適用する
- **THEN** `ReduceOutcome`、terminal state、activity state、wait state、queue intent はリファクタリング前と同等である
- **AND** resolve/reject wait queue の更新結果も同等である

#### Scenario: execution event の副作用が維持される

- **GIVEN** archive、merge、rejection review、resolve failure の代表的な `ExecutionEvent` がある
- **WHEN** event を reducer に適用する
- **THEN** display status と global invariant はリファクタリング前と同等である
- **AND** base-mutating lane の待機 queue は既存と同じ条件で追加・削除される
