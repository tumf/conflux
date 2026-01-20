## MODIFIED Requirements

### Requirement: Processing Item Spinner Animation

TUI は実行中の表示を processing ではなくフェーズ別の語彙で示さなければならない（SHALL）。apply の実行中は `applying`、acceptance 実行中は `accepting`、archive 実行中は `archiving`、resolve 実行中は `resolving`、依存待ちの change は `blocked` を表示すること。反復回数がある場合は `status:iteration` 形式で表示すること。

#### Scenario: Applying 状態の表示
- **GIVEN** TUI が running mode で change を処理している
- **WHEN** apply が実行中である
- **THEN** change のステータス表示は `applying` となる

#### Scenario: Resolving の iteration 表示
- **GIVEN** change の queue_status が resolving である
- **AND** iteration_number が 2 である
- **WHEN** TUI が change 行を表示する
- **THEN** ステータス表示は `resolving:2` となる

#### Scenario: 依存待ちの表示
- **GIVEN** change の queue_status が依存待ちである
- **WHEN** TUI が change 行を表示する
- **THEN** ステータス表示は `blocked` となる
