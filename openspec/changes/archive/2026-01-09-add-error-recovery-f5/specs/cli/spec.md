## ADDED Requirements

### Requirement: エラー状態の表示

エラー発生時、TUIはエラー状態を明示的に表示しなければならない（SHALL）。

#### Scenario: エラー発生時のモード遷移

- **WHEN** opencode実行がエラー（LLMエラー、料金不足等）で失敗する
- **THEN** TUIのモードが「Error」に遷移する
- **AND** ヘッダーのステータスが「Error」と赤色で表示される

#### Scenario: ステータスパネルのエラー表示

- **WHEN** TUIがエラー状態である
- **THEN** ステータスパネルに「Error in <change_id>」と表示される
- **AND** 「Press F5 to retry」のガイダンスが表示される

#### Scenario: エラー状態でのChange表示

- **WHEN** TUIがエラー状態である
- **THEN** エラーが発生したChangeのステータスは「[error]」と赤色で表示される
- **AND** 他のqueued状態のChangeはそのまま維持される

### Requirement: F5キーでのエラーリトライ

エラー状態でF5キーを押すと、エラーが発生したChangeの処理をリトライできなければならない（SHALL）。

#### Scenario: F5キーでリトライ開始

- **WHEN** TUIがエラー状態である
- **AND** ユーザーがF5キーを押す
- **THEN** エラー状態のChangeが再度キューに追加される
- **AND** TUIが「Running」モードに遷移する
- **AND** 処理が再開される

#### Scenario: リトライ時のログ表示

- **WHEN** ユーザーがF5キーでリトライを開始する
- **THEN** ログパネルに「Retrying: <change_id>」と表示される

#### Scenario: リトライ成功後の状態

- **WHEN** リトライした処理が成功する
- **THEN** Changeのステータスが「completed」または「archived」に更新される
- **AND** 残りのキュー内Changeがあれば続けて処理される

## MODIFIED Requirements

### Requirement: 実行モードダッシュボード

実行モードでは、処理中の変更の進捗状況をダッシュボード形式で表示しなければならない（SHALL）。

#### Scenario: 変更一覧の進捗表示

- **WHEN** TUIが実行モードである
- **THEN** 全ての変更がキュー状態と共に表示される
- **AND** 各変更の完了タスク数/総タスク数とパーセンテージが表示される

#### Scenario: キュー状態の表示

- **WHEN** TUIが実行モードである
- **THEN** 処理中の変更は進捗バーと共に表示される
- **AND** キュー待機中の変更は「queued」と表示される
- **AND** 未選択の変更は「not queued」と表示される
- **AND** エラーが発生した変更は「error」と赤色で表示される

#### Scenario: 現在処理中の変更のハイライト

- **WHEN** 変更が処理中である
- **THEN** 処理中の変更が視覚的にハイライトされる（`►` マーカー）
- **AND** ステータスパネルに変更IDと処理状況が表示される

#### Scenario: ログのリアルタイム表示

- **WHEN** オーケストレーションが実行中である
- **THEN** ログメッセージがログパネルにリアルタイムで追加される
- **AND** 最新のログが常に表示される（自動スクロール）

#### Scenario: 処理完了時の表示

- **WHEN** 全てのキュー内変更の処理が完了する
- **THEN** ヘッダーのステータスが「Completed」に変更される
- **AND** TUIは表示を維持し、ユーザーが `q` キーで終了できる

#### Scenario: エラー発生時の表示

- **WHEN** 変更の処理中にエラーが発生する
- **THEN** ヘッダーのステータスが「Error」と赤色で表示される
- **AND** ステータスパネルにエラー情報と「Press F5 to retry」が表示される
- **AND** TUIは表示を維持し、ユーザーがF5でリトライまたは`q`キーで終了できる
