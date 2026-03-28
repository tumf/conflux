## ADDED Requirements

### Requirement: Server Dashboard の変更ステータス表示語彙

server dashboard は、run モード dashboard と共通のステータス語彙で変更の状態を表示しなければならない（SHALL）。

ステータス語彙: `idle | queued | applying | accepting | archiving | resolving | archived | merged | error`

反復回数がある場合は `status:iteration` 形式（例: `applying:2`）で表示する。

#### Scenario: applying ステータスと iteration を表示する

- **GIVEN** server dashboard の Changes パネルが表示されている
- **AND** ある変更の status が `applying` で iteration_number が `2` である
- **WHEN** Changes パネルが更新される
- **THEN** そのエントリは `applying:2` と表示される

#### Scenario: archived ステータスを正しく表示する

- **GIVEN** server dashboard の Changes パネルが表示されている
- **AND** ある変更の status が `archived` である
- **WHEN** Changes パネルが表示される
- **THEN** そのエントリは `archived` と表示される
- **AND** `processing` や `completed` のような語彙は使用されない

### Requirement: Server Dashboard のログ配信表示

server dashboard は WebSocket 経由で受信したログを選択プロジェクトでフィルタリングして表示しなければならない（SHALL）。

#### Scenario: 選択プロジェクトのログのみ表示される

- **GIVEN** 複数プロジェクトが実行中で、それぞれログが配信されている
- **WHEN** ユーザーがプロジェクト A を選択する
- **THEN** Logs パネルにはプロジェクト A の `RemoteLogEntry` のみ表示される
- **AND** 他プロジェクトのログは表示されない

#### Scenario: 新規ログ受信時にオートスクロールする

- **GIVEN** Logs パネルが最下部にスクロールされている
- **WHEN** 新しい Log エントリが受信される
- **THEN** パネルは自動的に最新ログが表示されるよう下方向にスクロールする
