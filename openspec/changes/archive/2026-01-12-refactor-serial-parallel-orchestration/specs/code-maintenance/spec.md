## ADDED Requirements
### Requirement: Serial/Parallel 実行フローの共有化
システムは serial/parallel モードで共通となる apply・archive・進捗更新の処理を共有関数に集約しなければならない（SHALL）。

#### Scenario: serial/parallel が同じ共有関数を利用する
- **WHEN** serial モードで change を apply する
- **THEN** apply/archiving/進捗更新は共通関数経由で実行される
- **AND** parallel モードでも同じ共通関数が使用される

#### Scenario: モード固有の差分が分離される
- **WHEN** モード固有の出力やイベント送信を実装する
- **THEN** 共有関数は純粋な実行フローのみを扱う
- **AND** 出力/イベントの責務は呼び出し側に分離される
