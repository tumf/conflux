## MODIFIED Requirements

### Requirement: Queue ingestion and analysis targeting
並列実行の analysis は queued の change のみを対象にしなければならない（MUST）。

キューに追加された change は analysis 実行前に queued 集合へ反映されなければならない（MUST）。

queued の change が空の場合、analysis を実行してはならない（MUST）。

CLI の `run` サブコマンドでは、実行中の change が存在せず queued の change も空である場合、オーケストレーションは完了状態にならなければならない（MUST）。

通常の cflx ループ型実行では、ユーザが停止していない限り、実行中の change が存在せず queued の change も空であっても、オーケストレーション実行ループは終了してはならない（MUST NOT）。この状態では実行ループは待機を継続し、以後 queued になった change を検知したら analysis を再評価しなければならない（MUST）。

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

#### Scenario: CLI run は空キューで終了する
- **GIVEN** CLI の `run` サブコマンドで並列実行している
- **AND** 実行中の change が存在しない
- **AND** queued の change も空である
- **WHEN** 並列実行ループが次の analysis を開始しようとする
- **THEN** analysis を実行しない
- **AND** オーケストレーションは完了状態になる

#### Scenario: 通常の cflx 実行は空キューでも待機を継続する
- **GIVEN** `run` サブコマンド以外の通常の cflx ループ型実行である
- **AND** ユーザが停止していない
- **AND** 実行中の change が存在しない
- **AND** queued の change も空である
- **WHEN** 並列実行ループが次の analysis を開始しようとする
- **THEN** analysis を実行しない
- **AND** オーケストレーション実行ループは終了しない
- **AND** 新しく queued になった change を待機する

#### Scenario: 通常の cflx 実行の idle待機中のqueued追加でanalysisが再開する
- **GIVEN** `run` サブコマンド以外の通常の cflx ループ型実行である
- **AND** ユーザが停止していない
- **AND** 並列実行ループが queued 0 件・実行中 0 件で待機中である
- **WHEN** change が queued に追加される
- **THEN** 実行ループは queue 通知を受け取る
- **AND** analysis を再評価する

#### Scenario: queued外のchangeはanalysis対象から除外される
- **GIVEN** queued に含まれない change が存在する
- **AND** queued には別の change が存在する
- **WHEN** 並列実行が analysis を開始する
- **THEN** queued 外の change は analysis 対象から除外される
