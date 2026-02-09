## MODIFIED Requirements
### Requirement: Dynamic Execution Queue
Running 中に queued change を外した場合、当該 change がまだ Processing を開始していないなら、オーケストレータはその change を実行対象から除外しなければならない（MUST）。
Applying/Accepting/Archiving/Resolving の change は `Space` による単体停止要求のみ許可し、`@` による承認変更は受け付けてはならない（MUST NOT）。

#### Scenario: Running 中に queued change を外す
- **WHEN** TUI が Running モードである
- **AND** ユーザーが queued change を Space キーで NotQueued に切り替える
- **AND** その change が Processing を開始していない
- **THEN** その change は実行対象から除外される
- **AND** 以降の実行でその change は処理されない

#### Scenario: Running 中に実行中 change を単体停止する
- **GIVEN** TUI が Running モードである
- **AND** change の queue_status が Applying/Accepting/Archiving/Resolving のいずれかである
- **WHEN** ユーザーが Space キーを押す
- **THEN** 当該 change の停止要求が発行される
- **AND** 停止完了後に当該 change は `not queued` に戻り、実行マークが解除される
- **AND** 他の queued change は継続して処理される

#### Scenario: Processing 中の change で @ は無効
- **GIVEN** change の queue_status が Applying/Accepting/Archiving/Resolving のいずれかである
- **WHEN** ユーザーが `@` キーを押す
- **THEN** 承認状態と queue_status は変更されない
