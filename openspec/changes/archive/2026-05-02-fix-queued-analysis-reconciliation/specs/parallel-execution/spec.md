## MODIFIED Requirements

### Requirement: Re-analysis triggers and non-blocking scheduler

re-analysis は apply/acceptance/archive/resolve の in-flight が存在していても開始できなければならない（MUST）。

re-analysis ループは dispatch の完了待ちでブロックされてはならない（MUST NOT）。

re-analysis の起動トリガは、キュー通知・デバウンスタイマー・in-flight 完了・reducer-visible queued intent reconciliation のいずれでもよい（MUST）。

利用可能スロットが 0 の場合、システムは re-analysis を実行せず、空きができた時点で re-analysis を再評価しなければならない（MUST）。

スケジューラは reducer-visible queued work が存在するのに re-analysis を開始しない場合、その理由を観測可能なログまたはイベントとして出力しなければならない（SHALL）。

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
- **AND** no available slots の理由がログまたはイベントで観測できる

### Requirement: Queue ingestion and analysis targeting

並列実行の analysis は queued の change のみを対象にしなければならない（MUST）。

キューに追加された change は analysis 実行前に queued 集合へ反映されなければならない（MUST）。

scheduler-local queued 集合は reducer-visible queued intent と reconcile されなければならない（MUST）。reconcile は dynamic queue notification の欠落、dynamic queue pop 後の一時的な candidate load failure、または stale local queue state によって reducer-visible queued change が永続的に analysis 対象外になることを防がなければならない（MUST）。

queued の change が空の場合、analysis を実行してはならない（MUST）。ただし reducer-visible queued intent が存在する場合、scheduler は queued が空であると結論する前に reconcile を試みなければならない（MUST）。

実行中の change が存在せず、queued の change も空の場合、オーケストレーションは完了状態にならなければならない（MUST）。ただし reducer-visible queued intent が存在する場合、その intent が terminal / active / missing などの理由で analysis 対象外であることが確認されるまで完了状態として扱ってはならない（MUST NOT）。

queued に含まれない change（例: merged 済み change、実行済み change、削除済み change）は analysis 対象から除外されなければならない（MUST）。

#### Scenario: queuedのみがanalysis対象になる
- **GIVEN** queued に change が存在する
- **AND** queued 以外に実行中の change が存在する
- **WHEN** 並列実行が analysis を開始する
- **THEN** analysis 対象は queued の change のみになる

#### Scenario: reducer queued intent が scheduler-local queued に反映される
- **GIVEN** change `beta` has reducer-visible queued intent
- **AND** `beta` is not terminal
- **AND** `beta` is not active or in-flight
- **AND** `beta` can be loaded from active OpenSpec changes
- **AND** scheduler-local queued list does not contain `beta`
- **WHEN** the scheduler reconciles queued candidates before analysis
- **THEN** `beta` is added to scheduler-local queued candidates
- **AND** the next analysis includes `beta`

#### Scenario: dynamic queue notification miss is recoverable
- **GIVEN** change `beta` has reducer-visible queued intent
- **AND** the dynamic queue notification for `beta` was missed or already popped
- **AND** scheduler-local queued list does not contain `beta`
- **WHEN** the scheduler loop next reconciles queued candidates
- **THEN** `beta` is still eligible for analysis through reducer-visible queued intent
- **AND** `beta` does not remain indefinitely queued without analysis solely because the notification was missed

#### Scenario: candidate load failure is observable and retried
- **GIVEN** dynamic queue ingestion sees queued change id `beta`
- **AND** active OpenSpec change loading does not currently return `beta`
- **WHEN** scheduler ingestion skips `beta`
- **THEN** the skip reason is logged or emitted as candidate not found
- **AND** if reducer-visible queued intent for `beta` remains and `beta` later becomes loadable, reconciliation can add `beta` to analysis candidates

#### Scenario: queuedが空ならanalysisを実行しない
- **GIVEN** queued の change が存在しない
- **AND** reducer-visible queued intent も存在しない
- **WHEN** 並列実行が analysis を開始しようとする
- **THEN** analysis を実行しない

#### Scenario: 実行中とqueuedが空なら終了する
- **GIVEN** 実行中の change が存在しない
- **AND** queued の change も空である
- **AND** reducer-visible queued intent も存在しない
- **WHEN** 並列実行ループが次の analysis を開始しようとする
- **THEN** analysis を実行しない
- **AND** オーケストレーションは完了状態になる

#### Scenario: queued外のchangeはanalysis対象から除外される
- **GIVEN** queued に含まれない change が存在する
- **AND** queued には別の change が存在する
- **WHEN** 並列実行が analysis を開始する
- **THEN** queued 外の change は analysis 対象から除外される
