# cli Specification Delta

## MODIFIED Requirements

### Requirement: 実行モードダッシュボード

実行モードでは、処理中の変更の進捗状況をダッシュボード形式で表示しなければならない（SHALL）。

#### Scenario: キュー状態の表示

- **WHEN** TUIが実行モードである
- **THEN** 処理中の変更は進捗バーと共に表示される
- **AND** キュー待機中の変更は「queued」と表示される
- **AND** 未選択の変更は「not queued」と表示される
- **AND** エラーが発生した変更は「error」と赤色で表示される
- **AND** 全タスク完了した変更のみ「completed」と緑色で表示される

#### Scenario: Processing状態の維持（NEW）

- **WHEN** apply コマンドが成功する
- **AND** タスクが100%完了していない（completed_tasks < total_tasks）
- **THEN** 変更は「processing」状態を維持する
- **AND** 次のオーケストレーションループで再度処理される

#### Scenario: Completed状態への遷移（NEW）

- **WHEN** apply コマンドが成功する
- **AND** タスクが100%完了している（completed_tasks == total_tasks）
- **THEN** 変更は「completed」状態に遷移する
- **AND** アーカイブ処理が実行される

### Requirement: ProcessingCompleted イベントのタイミング

ProcessingCompleted イベントは、タスクが100%完了した場合にのみ発行されなければならない（SHALL）。

#### Scenario: Apply成功かつタスク完了

- **WHEN** apply コマンドが成功する
- **AND** タスクが100%完了している
- **THEN** ProcessingCompleted イベントが発行される
- **AND** 変更のキューステータスが Completed に更新される

#### Scenario: Apply成功かつタスク未完了

- **WHEN** apply コマンドが成功する
- **AND** タスクが100%完了していない
- **THEN** ProcessingCompleted イベントは発行されない
- **AND** 変更のキューステータスは Processing のまま維持される
- **AND** ログに「Apply iteration complete for <id>, continuing...」と記録される
