## MODIFIED Requirements

### Requirement: Parallel Analysis Targeting
並列実行のanalysisはqueuedのchangeのみを対象にしなければならない（MUST）。

実行中のchangeが存在せず、queuedのchangeも空の場合、システムはオーケストレーションを終了しなければならない（MUST）。

analysis対象をqueuedに限定するため、queuedに含まれないchange（例: merged済みchange、実行済みchange、削除済みchange）はanalysis対象から除外されなければならない（MUST）。

queuedのchangeが空の場合、analysisを実行してはならない（MUST）。

re-analysis は apply/acceptance/archive/resolve の in-flight が存在していても開始できなければならない（MUST）。

re-analysis ループは dispatch の完了待ちでブロックされてはならない（MUST NOT）。

システムは in-flight の change 数を追跡し、空きスロット数を算出しなければならない（MUST）。

re-analysis の起動トリガは、キュー通知・デバウンスタイマー・in-flight 完了のいずれでもよい（MUST）。

キューに追加された change は analysis 実行前に queued 集合へ反映されなければならない（MUST）。

スロットが空いていない場合でも re-analysis は実行でき、空きができた時点で次のディスパッチが行われなければならない（MUST）。

#### Scenario: queuedのみがanalysis対象になる
- **GIVEN** queuedにchangeが存在する
- **AND** queued以外に実行中のchangeが存在する
- **WHEN** 並列実行がanalysisを開始する
- **THEN** analysis対象はqueuedのchangeのみになる

#### Scenario: queued外のchangeはanalysis対象から除外される
- **GIVEN** queuedに含まれないchangeが存在する
- **AND** queuedには別のchangeが存在する
- **WHEN** 並列実行がanalysisを開始する
- **THEN** queued外のchangeはanalysis対象から除外される

#### Scenario: queuedが空ならanalysisを実行しない
- **GIVEN** queuedのchangeが存在しない
- **WHEN** 並列実行がanalysisを開始しようとする
- **THEN** analysisを実行しない

#### Scenario: 実行中とqueuedが空なら終了する
- **GIVEN** 実行中のchangeが存在しない
- **AND** queuedのchangeも空である
- **WHEN** 並列実行ループが次のanalysisを開始しようとする
- **THEN** analysisを実行しない
- **AND** オーケストレーションは完了状態になる

#### Scenario: キュー変化でre-analysisが起動する
- **GIVEN** 実行中のchangeが存在する
- **AND** queuedにchangeが追加される
- **WHEN** 並列実行がre-analysisを評価する
- **THEN** 完了イベントを待たずにre-analysisが開始される
- **AND** メインの実行ループ進行に依存しない

#### Scenario: in-flight 完了でre-analysisが再開する
- **GIVEN** apply/acceptance/archive/resolve の in-flight が存在する
- **AND** queuedに別のchangeが存在する
- **WHEN** in-flight の change が完了する
- **THEN** re-analysis が再評価される
- **AND** 空きスロット数に応じて次のchangeがディスパッチされる

#### Scenario: dispatch が re-analysis ループをブロックしない
- **GIVEN** in-flight の change が存在する
- **AND** queuedに別のchangeが存在する
- **WHEN** 並列実行がdispatchを開始する
- **THEN** re-analysis ループは apply 完了を待たずに次のトリガ待ちへ戻る

#### Scenario: スロットが空いていない場合でもre-analysisできる
- **GIVEN** 利用可能なスロットが0である
- **AND** queuedにchangeが存在する
- **WHEN** 並列実行がre-analysisを開始する
- **THEN** re-analysisは実行される
- **AND** スロットが空いた時点で次のchangeがディスパッチされる

#### Scenario: apply 実行中でもre-analysisが走る
- **GIVEN** apply 実行中の change が存在する
- **AND** queued に新しい change が追加される
- **WHEN** 並列実行が re-analysis を評価する
- **THEN** apply 完了を待たずに re-analysis が開始される
- **AND** 空きスロットができ次第、queued の change がディスパッチされる
