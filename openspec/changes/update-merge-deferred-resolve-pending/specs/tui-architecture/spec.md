## ADDED Requirements
### Requirement: MergeDeferred の待ち状態判定
TUI は `MergeDeferred` を受信したとき、resolve 実行中であれば対象 change を `ResolveWait` として扱い、resolve 待ち行列に追加しなければならない（SHALL）。
resolve が実行中でない場合、対象 change は `MergeWait` のまま保持されなければならない（SHALL）。

#### Scenario: resolve 実行中の MergeDeferred は ResolveWait になる
- **GIVEN** resolve 操作が進行中である
- **AND** change A が `MergeDeferred` を受信する
- **WHEN** TUI がイベントを処理する
- **THEN** change A のステータスは `ResolveWait` となる
- **AND** change A の change_id が resolve 待ち行列に追加される
- **AND** 表示語彙は `resolve pending` となる

#### Scenario: resolve 非実行時の MergeDeferred は MergeWait を維持する
- **GIVEN** resolve 操作が進行中ではない
- **AND** change A が `MergeDeferred` を受信する
- **WHEN** TUI がイベントを処理する
- **THEN** change A のステータスは `MergeWait` のまま維持される
