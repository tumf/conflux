## MODIFIED Requirements

### Requirement: Dynamic Execution Queue

Running 中に queued change を外した場合、当該 change がまだ Processing を開始していないなら、オーケストレータはその change を実行対象から除外しなければならない（MUST）。Processing/Archiving の change は引き続き操作できない。

#### Scenario: Running 中に queued change を外す
- **WHEN** TUI が Running モードである
- **AND** ユーザーが queued change を Space キーで NotQueued に切り替える
- **AND** その change が Processing を開始していない
- **THEN** その change は実行対象から除外される
- **AND** 以降の実行でその change は処理されない

#### Scenario: Processing 中の change は操作できない
- **WHEN** change が Processing または Archiving である
- **THEN** Space キーを押しても selected/queue 状態は変更されない
