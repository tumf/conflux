## MODIFIED Requirements

### Requirement: Re-analysis triggers and non-blocking scheduler
re-analysis は apply/acceptance/archive/resolve の in-flight が存在していても開始できなければならない（MUST）。

re-analysis ループは dispatch の完了待ちでブロックされてはならない（MUST NOT）。

re-analysis の起動トリガは、キュー通知・デバウンスタイマー・in-flight 完了のいずれでもよい（MUST）。

利用可能スロットが 0 の場合でも re-analysis は実行でき、空きができた時点で dispatch が行われなければならない（MUST）。

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

#### Scenario: スロットが空いていない場合でもre-analysisできる
- **GIVEN** 利用可能なスロットが0である
- **AND** queued に change が存在する
- **WHEN** 並列実行が re-analysis を開始する
- **THEN** re-analysis は実行される
- **AND** スロットが空いた時点で次の change が dispatch される

### Requirement: In-flight tracking and slot-based dispatch
システムは in-flight の change を追跡し、空きスロット数を算出しなければならない（MUST）。

in-flight は apply/acceptance/archive/resolve の change とし、merged/merge_wait/error/not queued を in-flight として扱ってはならない（MUST NOT）。

空きスロット数は `max_concurrent_workspaces - in_flight_count` で算出し、0 未満にならないように扱わなければならない（MUST）。

re-analysis の `order` は依存関係の制約として扱い、依存解決済みの change だけを空きスロット数分 dispatch しなければならない（MUST）。

#### Scenario: 空きスロット数に応じてdispatchする
- **GIVEN** `max_concurrent_workspaces` が 3 である
- **AND** in-flight が 2 件である
- **AND** queued に依存解決済みの change が 2 件ある
- **WHEN** re-analysis が dispatch を行う
- **THEN** 1 件のみ dispatch される

#### Scenario: in-flight に非アクティブ状態が含まれない
- **GIVEN** merged/merge_wait/error/not queued の change が存在する
- **WHEN** 並列実行が in-flight を算出する
- **THEN** それらの change は in-flight として数えられない

### Requirement: Queue ingestion and analysis targeting
並列実行の analysis は queued の change のみを対象にしなければならない（MUST）。

キューに追加された change は analysis 実行前に queued 集合へ反映されなければならない（MUST）。

queued の change が空の場合、analysis を実行してはならない（MUST）。

実行中の change が存在せず、queued の change も空の場合、オーケストレーションは完了状態にならなければならない（MUST）。

queued に含まれない change（例: merged 済み change、実行済み change、削除済み change）は analysis 対象から除外されなければならない（MUST）。

#### Scenario: queuedのみがanalysis対象になる
- **GIVEN** queued に change が存在する
- **AND** queued 以外に実行中の change が存在する
- **WHEN** 並列実行が analysis を開始する
- **THEN** analysis 対象は queued の change のみになる

#### Scenario: queuedが空ならanalysisを実行しない
- **GIVEN** queued の change が存在しない
- **WHEN** 並列実行が analysis を開始しようとする
- **THEN** analysis を実行しない

#### Scenario: 実行中とqueuedが空なら終了する
- **GIVEN** 実行中の change が存在しない
- **AND** queued の change も空である
- **WHEN** 並列実行ループが次の analysis を開始しようとする
- **THEN** analysis を実行しない
- **AND** オーケストレーションは完了状態になる

#### Scenario: queued外のchangeはanalysis対象から除外される
- **GIVEN** queued に含まれない change が存在する
- **AND** queued には別の change が存在する
- **WHEN** 並列実行が analysis を開始する
- **THEN** queued 外の change は analysis 対象から除外される

### Requirement: Dispatch sequencing for queued changes
キューに追加された change は analysis を経由せずに dispatch されてはならない（MUST NOT）。

dispatch は re-analysis ループのスケジューラによってのみ起動され、apply 側の補助ロジックから直接 spawn されてはならない（MUST）。

#### Scenario: 追加されたchangeはanalysis経由でdispatchされる
- **GIVEN** queued に新しい change が追加される
- **WHEN** 並列実行が次の dispatch を開始する
- **THEN** change は analysis の `order` に含まれている
- **AND** dispatch はスケジューラ経由でのみ起動される
