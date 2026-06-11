## MODIFIED Requirements

### Requirement: Re-analysis triggers and non-blocking scheduler

re-analysis は apply/acceptance/archive/resolve の in-flight が存在していても開始できなければならない（MUST）。

re-analysis ループは dispatch の完了待ちでブロックされてはならない（MUST NOT）。

re-analysis の起動トリガは、キュー通知・デバウンスタイマー・in-flight 完了・reducer-visible queued intent reconciliation のいずれでもよい（MUST）。

利用可能スロットが 0 の場合でも、queued に ordinary dispatchable candidate が存在するなら、システムは queue classification、reducer-visible queued intent reconciliation、dependency analysis、operator-visible diagnostics を実行できなければならない（MUST）。ただし ordinary apply dispatch は、dispatch 直前に再計算した利用可能スロットが 1 以上になるまで開始してはならない（MUST NOT）。

スケジューラは reducer-visible queued work が存在するのに re-analysis または dispatch を開始しない場合、その理由を観測可能なログまたはイベントとして出力しなければならない（SHALL）。

<!-- Expected canonical result after archive: `parallel-execution` will require re-analysis and diagnostics to remain active for queued dispatchable work during resolve, while final apply dispatch remains capacity-gated. -->

#### Scenario: キュー変化でre-analysisが起動する

- **GIVEN** apply 実行中の change が存在する
- **AND** queued に新しい change が追加される
- **WHEN** 並列実行が re-analysis を評価する
- **THEN** apply 完了を待たずに re-analysis が開始される

#### Scenario: reducer queued intentでre-analysisが起動する

- **GIVEN** reducer state に queued intent を持つ change が存在する
- **AND** scheduler-local queued list にはその change が存在しない
- **AND** 利用可能なスロットが1以上である
- **WHEN** 並列実行が re-analysis を評価する
- **THEN** scheduler は reducer-visible queued intent を analysis candidate に取り込む
- **AND** dynamic queue notification だけに依存せず re-analysis を開始する

#### Scenario: resolve中でもqueued candidateはanalysis対象になる

- **GIVEN** change A の resolve が進行中である
- **AND** resolve が現在の利用可能スロットを 0 にしている
- **AND** queued に ordinary dispatchable candidate change B が存在する
- **WHEN** 並列実行が re-analysis を評価する
- **THEN** scheduler は change B を analysis candidate として分類する
- **AND** dependency analysis を実行する
- **AND** change B の ordinary apply dispatch は開始しない
- **AND** dispatch が capacity gated である理由がログまたはイベントで観測できる

#### Scenario: resolve完了後にqueued candidateがdispatchされる

- **GIVEN** change A の resolve 中に change B が queued として分析済みである
- **AND** change B に未解決の依存 blocker がない
- **WHEN** change A の resolve が完了して利用可能スロットが 1 以上になる
- **THEN** scheduler は追加の queue notification やユーザー操作を待たずに re-analysis または slot-recovery dispatch evaluation を行う
- **AND** change B の ordinary apply dispatch を開始する

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

#### Scenario: スロットが空いていない場合はdispatchしない

- **GIVEN** 利用可能なスロットが0である
- **AND** queued に ordinary dispatchable candidate が存在する
- **WHEN** 並列実行が re-analysis と dispatch evaluation を行う
- **THEN** re-analysis は実行される
- **AND** ordinary apply dispatch は実行されない
- **AND** スロットが空いた時点で dispatch eligibility が再評価される
- **AND** no available slots または capacity gated の理由がログまたはイベントで観測できる
