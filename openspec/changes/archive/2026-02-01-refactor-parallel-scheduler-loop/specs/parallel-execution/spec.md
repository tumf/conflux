## MODIFIED Requirements
### Requirement: Re-analysis triggers and non-blocking scheduler
re-analysis は apply/acceptance/archive/resolve の in-flight が存在していても開始できなければならない（MUST）。

re-analysis ループは dispatch の完了待ちでブロックされてはならない（MUST NOT）。

re-analysis の起動トリガは、キュー通知・デバウンスタイマー・in-flight 完了のいずれでもよい（MUST）。

利用可能スロットが 0 の場合、システムは re-analysis を実行せず、空きができた時点で re-analysis を再評価しなければならない（MUST）。

スケジューラの実装は、再分析・ディスパッチ選定・完了処理の責務をヘルパー関数に分割してもよい（MAY）。ただし、非ブロッキング性と起動条件の挙動は維持しなければならない（MUST）。

#### Scenario: キュー変化でre-analysisが起動する
- **GIVEN** apply 実行中の change が存在する
- **AND** queued に新しい change が追加される
- **WHEN** 並列実行が re-analysis を評価する
- **THEN** apply 完了を待たずに re-analysis が開始される

#### Scenario: in-flight 完了でre-analysisが再開する
- **GIVEN** apply/acceptance/archive/resolve の in-flight が存在する
- **AND** queued に別の change が存在する
- **WHEN** in-flight の change が完了する
- **THEN** re-analysis が再評価される

#### Scenario: dispatch が re-analysis ループをブロックしない
- **GIVEN** in-flight の change が存在する
- **AND** queued に別の change が存在する
- **WHEN** 並列実行が dispatch を開始する
- **THEN** re-analysis ループは apply 完了を待たずに次のトリガ待ちへ戻る

#### Scenario: スロットが空いていない場合はre-analysisしない
- **GIVEN** 利用可能なスロットが0である
- **AND** queued に change が存在する
- **WHEN** 並列実行が re-analysis を評価する
- **THEN** re-analysis は実行されない
- **AND** スロットが空いた時点で re-analysis が再評価される
