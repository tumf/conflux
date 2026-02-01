## MODIFIED Requirements
### Requirement: Terminal Status Task Count Display

TUI は反復回数がある状態の表示を `status:iteration` 形式にしなければならない（SHALL）。apply/acceptance/archive/resolve の iteration 番号が更新された場合、Changes 一覧のステータス表示は最新の iteration に同期し続けなければならない（SHALL）。Applying中のChanges行では、ステータスは`[status:iteration]`のみを表示し、タスク進捗は`<completed>/<total>(<percent>%)`形式で表示しなければならない（SHALL）。

#### Scenario: Applying の iteration 表示
- **GIVEN** change が apply 実行中である
- **AND** apply の iteration 番号が 1 である
- **WHEN** TUI が change 行を表示する
- **THEN** ステータス表示は `applying:1` となる

#### Scenario: Archiving の iteration 表示
- **GIVEN** change が archive 実行中である
- **AND** archive の iteration 番号が 2 である
- **WHEN** TUI が change 行を表示する
- **THEN** ステータス表示は `archiving:2` となる

#### Scenario: Applying の iteration 更新に追従する
- **GIVEN** change の queue_status が applying である
- **AND** iteration_number が 2 から 3 に更新される
- **WHEN** TUI が Changes 一覧を再描画する
- **THEN** ステータス表示は `applying:3` となる

#### Scenario: Applying の進捗表示フォーマット
- **GIVEN** change の queue_status が applying である
- **AND** iteration 番号が 1 である
- **AND** completed_tasks が 0 で total_tasks が 3 である
- **WHEN** TUI が change 行を表示する
- **THEN** Changes 行の進捗表示は `0/3(0%)` となる
