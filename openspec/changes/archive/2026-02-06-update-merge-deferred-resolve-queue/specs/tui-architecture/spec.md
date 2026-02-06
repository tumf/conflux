## MODIFIED Requirements
### Requirement: MergeDeferred の待ち状態判定
TUI は `MergeDeferred` を受信したとき、resolve 実行中であり対象 change が現在 resolve 中の change ではない場合、対象 change を `ResolveWait` として扱い、resolve 待ち行列に追加しなければならない（SHALL）。
resolve 実行中で対象 change が現在 resolve 中の change と同一である場合、対象 change は `Resolving` のまま維持され、resolve 待ち行列に追加されてはならない（SHALL NOT）。
resolve が実行中でない場合、対象 change は `MergeWait` のまま保持されなければならない（SHALL）。

#### Scenario: resolve 実行中の MergeDeferred は ResolveWait になる
- **GIVEN** resolve 操作が進行中である
- **AND** change A が `MergeDeferred` を受信する
- **AND** change A は現在 resolve 中の change ではない
- **WHEN** TUI がイベントを処理する
- **THEN** change A のステータスは `ResolveWait` となる
- **AND** change A の change_id が resolve 待ち行列に追加される
- **AND** 表示語彙は `resolve pending` となる

#### Scenario: resolve 実行中の MergeDeferred が現在 resolve 中の change の場合は自己キューしない
- **GIVEN** resolve 操作が進行中である
- **AND** change A が現在 resolve 中の change である
- **AND** change A が `MergeDeferred` を受信する
- **WHEN** TUI がイベントを処理する
- **THEN** change A のステータスは `Resolving` のまま維持される
- **AND** change A の change_id は resolve 待ち行列に追加されない

#### Scenario: resolve 非実行時の MergeDeferred は MergeWait を維持する
- **GIVEN** resolve 操作が進行中ではない
- **AND** change A が `MergeDeferred` を受信する
- **WHEN** TUI がイベントを処理する
- **THEN** change A のステータスは `MergeWait` のまま維持される
