## MODIFIED Requirements

### Requirement: Processing Item Spinner Animation

TUI は実行中の表示を processing ではなくフェーズ別の語彙で示さなければならない（SHALL）。apply の実行中は `applying`、acceptance 実行中は `accepting`、archive 実行中は `archiving`、resolve 実行中は `resolving` を表示すること。

#### Scenario: Applying 状態の表示
- **GIVEN** TUI が running mode で change を処理している
- **WHEN** apply が実行中である
- **THEN** change のステータス表示は `applying` となる

#### Scenario: Accepting 状態の表示
- **GIVEN** TUI が running mode で change を処理している
- **WHEN** acceptance が実行中である
- **THEN** change のステータス表示は `accepting` となる

#### Scenario: Archiving 状態の表示
- **GIVEN** TUI が running mode で change を処理している
- **WHEN** archive が実行中である
- **THEN** change のステータス表示は `archiving` となる

#### Scenario: Resolving 状態の表示
- **GIVEN** TUI が running mode で change を処理している
- **WHEN** resolve が実行中である
- **THEN** change のステータス表示は `resolving` となる

### Requirement: Terminal Status Task Count Display

TUI は反復回数がある状態の表示を `status:iteration` 形式にしなければならない（SHALL）。

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
